use axum::{extract::{State, Extension, Path, Query}, Json};
use uuid::Uuid;
use rust_decimal::Decimal;
use serde::Deserialize;
use crate::{auth::models::AuthenticatedUser, errors::{AppError, AppResult}, middleware::permission::require_permission, routes::AppState};

#[derive(Deserialize)]
pub struct PurchaseListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub status: Option<String>,
    pub supplier_id: Option<Uuid>,
}

pub async fn list_purchases(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(q): Query<PurchaseListQuery>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "purchases.read")?;
    let page = q.page.unwrap_or(1).max(1);
    let per_page = q.per_page.unwrap_or(20).min(100);
    let offset = (page - 1) * per_page;

    let rows = sqlx::query!(
        r#"SELECT po.id, po.po_number, po.purchase_date, po.grand_total,
                  po.advance_paid, po.outstanding_amount, po.status, po.payment_status,
                  po.due_date, s.business_name as supplier_name
           FROM purchase_orders po
           JOIN suppliers s ON s.id = po.supplier_id
           WHERE ($1::text IS NULL OR po.status = $1)
             AND ($2::uuid IS NULL OR po.supplier_id = $2)
           ORDER BY po.purchase_date DESC
           LIMIT $3 OFFSET $4"#,
        q.status, q.supplier_id, per_page, offset
    ).fetch_all(&state.pool).await?;

    let total: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM purchase_orders")
        .fetch_one(&state.pool).await?.unwrap_or(0);

    let data: Vec<_> = rows.iter().map(|r| serde_json::json!({
        "id": r.id, "po_number": r.po_number, "supplier_name": r.supplier_name,
        "purchase_date": r.purchase_date, "grand_total": r.grand_total,
        "advance_paid": r.advance_paid, "outstanding_amount": r.outstanding_amount,
        "status": r.status, "payment_status": r.payment_status, "due_date": r.due_date
    })).collect();

    Ok(Json(serde_json::json!({"data": data, "total": total, "page": page, "per_page": per_page})))
}

pub async fn get_purchase(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "purchases.read")?;

    let row = sqlx::query!(
        r#"SELECT po.*, s.business_name as supplier_name
           FROM purchase_orders po JOIN suppliers s ON s.id = po.supplier_id
           WHERE po.id = $1"#, id
    ).fetch_optional(&state.pool).await?
     .ok_or_else(|| AppError::NotFound("Purchase not found".into()))?;

    let items = sqlx::query!(
        r#"SELECT poi.*, p.name as product_name, p.sku
           FROM purchase_order_items poi JOIN products p ON p.id = poi.product_id
           WHERE poi.purchase_order_id = $1"#, id
    ).fetch_all(&state.pool).await?;

    let item_data: Vec<_> = items.iter().map(|i| serde_json::json!({
        "id": i.id, "product_name": i.product_name, "sku": i.sku,
        "quantity": i.quantity, "unit_cost": i.unit_cost, "gst_rate": i.gst_rate,
        "total": i.total, "received_quantity": i.received_quantity
    })).collect();

    Ok(Json(serde_json::json!({
        "id": row.id, "po_number": row.po_number, "supplier_name": row.supplier_name,
        "purchase_date": row.purchase_date, "grand_total": row.grand_total,
        "advance_paid": row.advance_paid, "outstanding_amount": row.outstanding_amount,
        "status": row.status, "payment_status": row.payment_status,
        "due_date": row.due_date, "notes": row.notes, "items": item_data
    })))
}

pub async fn create_purchase(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "purchases.create")?;

    let supplier_id: Uuid = body.get("supplier_id").and_then(|v| v.as_str()).and_then(|s| s.parse().ok())
        .ok_or_else(|| AppError::Validation("supplier_id required".into()))?;

    let grand_total: Decimal = body.get("grand_total")
        .and_then(|v| v.as_str().and_then(|s| s.parse().ok()).or_else(|| v.as_f64().map(|f| Decimal::try_from(f).unwrap_or(Decimal::ZERO))))
        .unwrap_or(Decimal::ZERO);

    let advance: Decimal = body.get("advance_paid")
        .and_then(|v| v.as_str().and_then(|s| s.parse().ok()).or_else(|| v.as_f64().map(|f| Decimal::try_from(f).unwrap_or(Decimal::ZERO))))
        .unwrap_or(Decimal::ZERO);

    let outstanding = grand_total - advance;
    let terms_days: i32 = body.get("payment_terms_days").and_then(|v| v.as_i64()).map(|v| v as i32).unwrap_or(30);

    let po_number = generate_po_number(&state.pool).await?;

    let mut tx = state.pool.begin().await?;

    let id = sqlx::query_scalar!(
        r#"INSERT INTO purchase_orders (
            po_number, supplier_id, purchase_date, grand_total,
            advance_paid, outstanding_amount, payment_terms_days,
            due_date, status, payment_status, created_by,
            supplier_invoice_number, notes
        ) VALUES ($1,$2,CURRENT_DATE,$3,$4,$5,$6,
            CURRENT_DATE + $6::integer * INTERVAL '1 day',
            'draft','unpaid',$7,$8,$9)
        RETURNING id"#,
        po_number, supplier_id, grand_total, advance, outstanding, terms_days, user.id,
        body.get("supplier_invoice_number").and_then(|v| v.as_str()),
        body.get("notes").and_then(|v| v.as_str())
    ).fetch_one(&mut *tx).await?;

    // Insert items
    if let Some(items) = body.get("items").and_then(|v| v.as_array()) {
        for item in items {
            let product_id: Uuid = item.get("product_id")
                .and_then(|v| v.as_str()).and_then(|s| s.parse().ok())
                .ok_or_else(|| AppError::Validation("item.product_id required".into()))?;

            // Accept both string and number for numeric fields
            let parse_decimal = |val: Option<&serde_json::Value>| -> Decimal {
                val.and_then(|v| {
                    v.as_str().and_then(|s| s.parse().ok())
                    .or_else(|| v.as_f64().and_then(|f| Decimal::try_from(f).ok()))
                    .or_else(|| v.as_i64().map(|i| Decimal::from(i)))
                }).unwrap_or(Decimal::ZERO)
            };

            let qty = {
                let v = parse_decimal(item.get("quantity"));
                if v <= Decimal::ZERO { Decimal::ONE } else { v }
            };
            let unit_cost = parse_decimal(item.get("unit_cost"));
            let gst_rate = {
                let v = parse_decimal(item.get("gst_rate"));
                if v <= Decimal::ZERO { Decimal::from(18) } else { v }
            };
            let taxable = qty * unit_cost;
            let gst_amt = taxable * gst_rate / Decimal::from(100);
            let total = taxable + gst_amt;

            sqlx::query!(
                r#"INSERT INTO purchase_order_items (purchase_order_id, product_id, quantity, unit_cost, taxable_value, gst_rate, gst_amount, total)
                   VALUES ($1,$2,$3,$4,$5,$6,$7,$8)"#,
                id, product_id, qty, unit_cost, taxable, gst_rate, gst_amt, total
            ).execute(&mut *tx).await?;
        }
    }

    // If advance > 0, record it as a payment
    if advance > Decimal::ZERO {
        sqlx::query!(
            r#"INSERT INTO payments (amount, payment_method, direction, reference_type, reference_id, supplier_id, status, recorded_by, notes)
               VALUES ($1, 'cash', 'outbound', 'purchase', $2, $3, 'verified', $4, 'Advance payment')"#,
            advance, id, supplier_id, user.id
        ).execute(&mut *tx).await?;
    }

    // Schedule reminder
    schedule_payment_reminder(&mut tx, id, outstanding, terms_days).await?;

    tx.commit().await?;

    Ok(Json(serde_json::json!({"id": id, "po_number": po_number, "message": "Purchase order created"})))
}

pub async fn update_purchase(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "purchases.create")?;
    sqlx::query!(
        "UPDATE purchase_orders SET notes = COALESCE($1, notes), updated_at = NOW() WHERE id = $2",
        body.get("notes").and_then(|v| v.as_str()), id
    ).execute(&state.pool).await?;
    Ok(Json(serde_json::json!({"message": "Updated"})))
}

pub async fn approve_purchase(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "purchases.approve")?;
    sqlx::query!(
        "UPDATE purchase_orders SET status = 'approved', updated_at = NOW() WHERE id = $1 AND status = 'draft'",
        id
    ).execute(&state.pool).await?;
    Ok(Json(serde_json::json!({"message": "Purchase approved"})))
}

pub async fn receive_goods(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "purchases.receive")?;

    let location_id: Uuid = body.get("location_id").and_then(|v| v.as_str()).and_then(|s| s.parse().ok())
        .ok_or_else(|| AppError::Validation("location_id required".into()))?;

    let mut tx = state.pool.begin().await?;

    let items = body.get("items").and_then(|v| v.as_array()).cloned().unwrap_or_default();

    for item in &items {
        let product_id: Uuid = item.get("product_id").and_then(|v| v.as_str()).and_then(|s| s.parse().ok())
            .ok_or_else(|| AppError::Validation("item.product_id required".into()))?;
        let qty: Decimal = item.get("received_quantity").and_then(|v| v.as_str()).and_then(|s| s.parse().ok())
            .unwrap_or(Decimal::ZERO);

        if qty <= Decimal::ZERO { continue; }

        // Update inventory balance
        sqlx::query!(
            r#"INSERT INTO inventory_balances (product_id, location_id, on_hand)
               VALUES ($1, $2, $3)
               ON CONFLICT (product_id, location_id) DO UPDATE
               SET on_hand = inventory_balances.on_hand + $3"#,
            product_id, location_id, qty
        ).execute(&mut *tx).await?;

        // Record stock movement
        sqlx::query!(
            r#"INSERT INTO stock_movements (product_id, quantity, destination_location_id, movement_type, reference_document, user_id)
               VALUES ($1, $2, $3, 'purchase_receipt', $4, $5)"#,
            product_id, qty, location_id, id.to_string(), user.id
        ).execute(&mut *tx).await?;

        // Update received qty on order item
        sqlx::query!(
            "UPDATE purchase_order_items SET received_quantity = received_quantity + $1 WHERE purchase_order_id = $2 AND product_id = $3",
            qty, id, product_id
        ).execute(&mut *tx).await?;
    }

    // Update purchase status
    sqlx::query!(
        "UPDATE purchase_orders SET status = 'partially_received', updated_at = NOW() WHERE id = $1 AND status IN ('approved', 'ordered')",
        id
    ).execute(&mut *tx).await?;

    tx.commit().await?;

    Ok(Json(serde_json::json!({"message": "Goods received and inventory updated"})))
}

async fn generate_po_number(pool: &crate::database::DbPool) -> AppResult<String> {
    let count: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM purchase_orders")
        .fetch_one(pool).await?.unwrap_or(0);
    Ok(format!("PO-{:06}", count + 1))
}

async fn schedule_payment_reminder(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    purchase_id: Uuid,
    outstanding: Decimal,
    terms_days: i32,
) -> AppResult<()> {
    if outstanding <= Decimal::ZERO { return Ok(()); }

    // Schedule reminder 2 days before due date
    sqlx::query!(
        r#"INSERT INTO payment_reminders (reference_type, reference_id, due_date, outstanding_amount, status, reminder_days_before)
           VALUES ('purchase', $1, CURRENT_DATE + $2::integer * INTERVAL '1 day', $3, 'scheduled', 2)"#,
        purchase_id, terms_days, outstanding
    ).execute(&mut **tx).await?;

    Ok(())
}
