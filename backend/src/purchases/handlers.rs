use axum::{
    extract::{Extension, Path, Query, State},
    Json,
};
use rust_decimal::Decimal;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    audit::service::log_action,
    auth::models::AuthenticatedUser,
    common::{dec, dec_field, int, money, next_document_number, non_empty, paginated, payment_method, text, today, uuid, ListQuery},
    errors::{AppError, AppResult},
    inventory::stock,
    middleware::permission::require_permission,
    routes::AppState,
};

pub async fn list_purchases(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "purchases.read")?;
    let search = q.search();
    let status = q.status();
    let payment_status = non_empty(&q.payment_status);

    let rows = sqlx::query!(
        r#"SELECT po.id, po.po_number, po.supplier_id, po.supplier_invoice_number, po.purchase_date,
                  po.grand_total, po.advance_paid, po.outstanding_amount, po.status, po.payment_status,
                  po.due_date, po.created_at, s.business_name AS supplier_name
           FROM purchase_orders po
           JOIN suppliers s ON s.id = po.supplier_id
           WHERE ($1::text IS NULL OR po.status = $1)
             AND ($2::uuid IS NULL OR po.supplier_id = $2)
             AND ($3::text IS NULL OR po.payment_status = $3)
             AND ($4::text IS NULL OR po.po_number ILIKE '%' || $4 || '%' OR s.business_name ILIKE '%' || $4 || '%'
                  OR po.supplier_invoice_number ILIKE '%' || $4 || '%')
             AND ($5::date IS NULL OR po.purchase_date >= $5)
             AND ($6::date IS NULL OR po.purchase_date <= $6)
           ORDER BY po.purchase_date DESC, po.created_at DESC
           LIMIT $7 OFFSET $8"#,
        status, q.supplier_id, payment_status, search, q.from, q.to, q.per_page(), q.offset()
    )
    .fetch_all(&state.pool)
    .await?;

    let total: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM purchase_orders po JOIN suppliers s ON s.id = po.supplier_id
           WHERE ($1::text IS NULL OR po.status = $1)
             AND ($2::uuid IS NULL OR po.supplier_id = $2)
             AND ($3::text IS NULL OR po.payment_status = $3)
             AND ($4::text IS NULL OR po.po_number ILIKE '%' || $4 || '%' OR s.business_name ILIKE '%' || $4 || '%'
                  OR po.supplier_invoice_number ILIKE '%' || $4 || '%')
             AND ($5::date IS NULL OR po.purchase_date >= $5)
             AND ($6::date IS NULL OR po.purchase_date <= $6)"#,
        status, q.supplier_id, payment_status, search, q.from, q.to
    )
    .fetch_one(&state.pool)
    .await?;

    let data = rows
        .into_iter()
        .map(|r| {
            json!({
                "id": r.id, "po_number": r.po_number, "supplier_id": r.supplier_id, "supplier_name": r.supplier_name,
                "supplier_invoice_number": r.supplier_invoice_number,
                "purchase_date": r.purchase_date, "grand_total": r.grand_total,
                "advance_paid": r.advance_paid, "amount_paid": r.advance_paid,
                "outstanding_amount": r.outstanding_amount,
                "status": r.status, "payment_status": r.payment_status, "due_date": r.due_date,
                "created_at": r.created_at,
            })
        })
        .collect();

    Ok(Json(paginated(data, total, &q)))
}

pub async fn get_purchase(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "purchases.read")?;

    let row = sqlx::query!(
        r#"SELECT po.*, s.business_name AS supplier_name, s.phone AS supplier_phone, s.gstin AS supplier_gstin,
                  u.full_name AS "created_by_name?"
           FROM purchase_orders po
           JOIN suppliers s ON s.id = po.supplier_id
           LEFT JOIN users u ON u.id = po.created_by
           WHERE po.id = $1"#,
        id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Purchase order not found".into()))?;

    let items = sqlx::query!(
        r#"SELECT poi.*, p.name AS product_name, p.sku
           FROM purchase_order_items poi JOIN products p ON p.id = poi.product_id
           WHERE poi.purchase_order_id = $1 ORDER BY p.name"#,
        id
    )
    .fetch_all(&state.pool)
    .await?;

    let payments = sqlx::query!(
        r#"SELECT id, payment_date, amount, payment_method, transaction_reference, notes, created_at
           FROM payments WHERE reference_type = 'purchase' AND reference_id = $1 ORDER BY created_at"#,
        id
    )
    .fetch_all(&state.pool)
    .await?;

    let item_data: Vec<Value> = items
        .iter()
        .map(|i| {
            json!({
                "id": i.id, "product_id": i.product_id, "product_name": i.product_name, "sku": i.sku,
                "quantity": i.quantity, "unit_cost": i.unit_cost, "discount_percent": i.discount_percent,
                "taxable_value": i.taxable_value, "gst_rate": i.gst_rate, "gst_amount": i.gst_amount,
                "total": i.total, "received_quantity": i.received_quantity,
                "pending_quantity": (i.quantity - i.received_quantity).max(Decimal::ZERO),
            })
        })
        .collect();

    Ok(Json(json!({
        "id": row.id, "po_number": row.po_number,
        "supplier_id": row.supplier_id, "supplier_name": row.supplier_name,
        "supplier_phone": row.supplier_phone, "supplier_gstin": row.supplier_gstin,
        "supplier_invoice_number": row.supplier_invoice_number, "invoice_date": row.invoice_date,
        "purchase_date": row.purchase_date,
        "subtotal": row.subtotal, "discount_amount": row.discount_amount, "taxable_value": row.taxable_value,
        "gst_amount": row.gst_amount, "freight": row.freight, "other_charges": row.other_charges,
        "grand_total": row.grand_total,
        "advance_paid": row.advance_paid, "amount_paid": row.advance_paid,
        "outstanding_amount": row.outstanding_amount,
        "payment_terms_days": row.payment_terms_days, "due_date": row.due_date,
        "status": row.status, "payment_status": row.payment_status,
        "notes": row.notes, "created_by_name": row.created_by_name, "created_at": row.created_at,
        "items": item_data,
        "payments": payments.iter().map(|p| json!({
            "id": p.id, "payment_date": p.payment_date, "amount": p.amount, "payment_method": p.payment_method,
            "transaction_reference": p.transaction_reference, "notes": p.notes, "created_at": p.created_at
        })).collect::<Vec<_>>(),
    })))
}

fn payment_status_for(paid: Decimal, outstanding: Decimal) -> &'static str {
    if outstanding <= Decimal::ZERO {
        "paid"
    } else if paid > Decimal::ZERO {
        "partially_paid"
    } else {
        "unpaid"
    }
}

pub async fn create_purchase(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(body): Json<Value>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "purchases.create")?;

    let supplier_id = uuid(body.get("supplier_id")).ok_or_else(|| AppError::Validation("Select a supplier".into()))?;
    let items = body.get("items").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    if items.is_empty() {
        return Err(AppError::Validation("Add at least one item to the purchase order".into()));
    }

    let freight = money(dec_field(&body, "freight")?.unwrap_or(Decimal::ZERO));
    let other_charges = money(dec_field(&body, "other_charges")?.unwrap_or(Decimal::ZERO));
    let advance = money(dec_field(&body, "advance_paid")?.unwrap_or(Decimal::ZERO));
    if freight < Decimal::ZERO || other_charges < Decimal::ZERO || advance < Decimal::ZERO {
        return Err(AppError::Validation("Charges and advance cannot be negative".into()));
    }
    let advance_method = payment_method(body.get("payment_method"))?;

    let mut tx = state.pool.begin().await?;

    let supplier = sqlx::query!(
        "SELECT business_name, payment_terms_days, is_active FROM suppliers WHERE id = $1 FOR UPDATE",
        supplier_id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::Validation("Supplier not found".into()))?;
    if !supplier.is_active {
        return Err(AppError::BusinessLogic(format!("Supplier {} is inactive", supplier.business_name)));
    }
    let terms_days = int(body.get("payment_terms_days")).map(|d| d as i32).unwrap_or(supplier.payment_terms_days).max(0);

    let hundred = Decimal::from(100);
    struct Line { product_id: Uuid, qty: Decimal, unit_cost: Decimal, discount: Decimal, taxable: Decimal, gst_rate: Decimal, gst: Decimal }
    let mut lines = Vec::with_capacity(items.len());
    for (idx, item) in items.iter().enumerate() {
        let n = idx + 1;
        let product_id = uuid(item.get("product_id")).ok_or_else(|| AppError::Validation(format!("Line {}: select a product", n)))?;
        let product = sqlx::query!("SELECT name, gst_rate, purchase_price FROM products WHERE id = $1", product_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::Validation(format!("Line {}: product not found", n)))?;

        let qty = dec(item.get("quantity")).unwrap_or(Decimal::ZERO);
        if qty <= Decimal::ZERO {
            return Err(AppError::Validation(format!("{}: quantity must be greater than zero", product.name)));
        }
        let unit_cost = dec(item.get("unit_cost")).unwrap_or(product.purchase_price);
        if unit_cost < Decimal::ZERO {
            return Err(AppError::Validation(format!("{}: cost cannot be negative", product.name)));
        }
        let discount = dec(item.get("discount_percent")).unwrap_or(Decimal::ZERO);
        if discount < Decimal::ZERO || discount > hundred {
            return Err(AppError::Validation(format!("{}: discount must be between 0 and 100%", product.name)));
        }
        let gst_rate = dec(item.get("gst_rate")).unwrap_or(product.gst_rate);
        if gst_rate < Decimal::ZERO || gst_rate > hundred {
            return Err(AppError::Validation(format!("{}: invalid GST rate", product.name)));
        }
        let gross = money(qty * unit_cost);
        let taxable = gross - money(gross * discount / hundred);
        let gst = money(taxable * gst_rate / hundred);
        lines.push(Line { product_id, qty, unit_cost, discount, taxable, gst_rate, gst });
    }

    let subtotal: Decimal = lines.iter().map(|l| money(l.qty * l.unit_cost)).sum();
    let taxable_total: Decimal = lines.iter().map(|l| l.taxable).sum();
    let gst_total: Decimal = lines.iter().map(|l| l.gst).sum();
    let grand_total = taxable_total + gst_total + freight + other_charges;
    if advance > grand_total {
        return Err(AppError::Validation(format!("Advance ({}) cannot exceed the order total ({})", advance, grand_total)));
    }
    let outstanding = grand_total - advance;

    let purchase_date = today();
    let due_date = purchase_date + chrono::Duration::days(terms_days as i64);
    let po_number = next_document_number(&mut tx, "purchase_orders", "po_number", &format!("PO-{}", purchase_date.format("%Y"))).await?;

    let id = sqlx::query_scalar!(
        r#"INSERT INTO purchase_orders (
            po_number, supplier_id, supplier_invoice_number, purchase_date, subtotal, discount_amount,
            taxable_value, gst_amount, freight, other_charges, grand_total,
            advance_paid, outstanding_amount, payment_terms_days, due_date, status, payment_status,
            created_by, notes
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,'draft',$16,$17,$18)
        RETURNING id"#,
        po_number, supplier_id, text(body.get("supplier_invoice_number")), purchase_date,
        subtotal, subtotal - taxable_total, taxable_total, gst_total, freight, other_charges, grand_total,
        advance, outstanding, terms_days, due_date, payment_status_for(advance, outstanding),
        user.id, text(body.get("notes"))
    )
    .fetch_one(&mut *tx)
    .await?;

    for l in &lines {
        sqlx::query!(
            r#"INSERT INTO purchase_order_items (purchase_order_id, product_id, quantity, unit_cost, discount_percent,
                   taxable_value, gst_rate, gst_amount, total)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)"#,
            id, l.product_id, l.qty, l.unit_cost, l.discount, l.taxable, l.gst_rate, l.gst, l.taxable + l.gst
        )
        .execute(&mut *tx)
        .await?;
    }

    if advance > Decimal::ZERO {
        sqlx::query!(
            r#"INSERT INTO payments (payment_date, amount, payment_method, direction, reference_type, reference_id,
                   supplier_id, status, recorded_by, notes)
               VALUES ($1, $2, $3, 'outbound', 'purchase', $4, $5, 'verified', $6, 'Advance payment')"#,
            purchase_date, advance, advance_method, id, supplier_id, user.id
        )
        .execute(&mut *tx)
        .await?;
    }

    if outstanding > Decimal::ZERO {
        sqlx::query!(
            "UPDATE suppliers SET outstanding_balance = outstanding_balance + $1 WHERE id = $2",
            outstanding, supplier_id
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            r#"INSERT INTO payment_reminders (reference_type, reference_id, due_date, outstanding_amount, status, reminder_days_before)
               VALUES ('purchase', $1, $2, $3, 'scheduled', 2)"#,
            id, due_date, outstanding
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    log_action(&state.pool, user.id, "purchase_created", "purchase_order", Some(&id.to_string()),
        Some(json!({"po_number": po_number, "grand_total": grand_total}))).await;

    Ok(Json(json!({"id": id, "po_number": po_number, "grand_total": grand_total, "message": "Purchase order created"})))
}

pub async fn update_purchase(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
    Json(body): Json<Value>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "purchases.create")?;
    let invoice_date = body
        .get("invoice_date")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<chrono::NaiveDate>())
        .transpose()
        .map_err(|_| AppError::Validation("invoice_date must be YYYY-MM-DD".into()))?;

    let res = sqlx::query!(
        r#"UPDATE purchase_orders SET
               notes = COALESCE($1, notes),
               supplier_invoice_number = COALESCE($2, supplier_invoice_number),
               invoice_date = COALESCE($3, invoice_date),
               updated_at = NOW()
           WHERE id = $4"#,
        text(body.get("notes")), text(body.get("supplier_invoice_number")), invoice_date, id
    )
    .execute(&state.pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound("Purchase order not found".into()));
    }
    Ok(Json(json!({"message": "Purchase order updated"})))
}

pub async fn approve_purchase(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "purchases.approve")?;
    let res = sqlx::query!(
        "UPDATE purchase_orders SET status = 'approved', updated_at = NOW() WHERE id = $1 AND status = 'draft'",
        id
    )
    .execute(&state.pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::BusinessLogic("Only draft purchase orders can be approved".into()));
    }
    log_action(&state.pool, user.id, "purchase_approved", "purchase_order", Some(&id.to_string()), None).await;
    Ok(Json(json!({"message": "Purchase order approved"})))
}

pub async fn cancel_purchase(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
    Json(body): Json<Value>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "purchases.approve")?;
    let reason = text(body.get("reason")).ok_or_else(|| AppError::Validation("A cancellation reason is required".into()))?;

    let mut tx = state.pool.begin().await?;
    let po = sqlx::query!(
        "SELECT po_number, status, supplier_id, advance_paid, outstanding_amount FROM purchase_orders WHERE id = $1 FOR UPDATE",
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Purchase order not found".into()))?;

    if !matches!(po.status.as_str(), "draft" | "approved" | "ordered") {
        return Err(AppError::BusinessLogic(format!("A {} purchase order cannot be cancelled", po.status.replace('_', " "))));
    }
    if po.advance_paid > Decimal::ZERO {
        return Err(AppError::BusinessLogic("Payments have been made against this order; settle them with the supplier first".into()));
    }

    sqlx::query!(
        "UPDATE suppliers SET outstanding_balance = GREATEST(outstanding_balance - $1, 0) WHERE id = $2",
        po.outstanding_amount, po.supplier_id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        r#"UPDATE purchase_orders SET status = 'cancelled', outstanding_amount = 0, updated_at = NOW(),
               notes = CONCAT_WS(E'\n', NULLIF(notes, ''), 'Cancelled: ' || $2::text)
           WHERE id = $1"#,
        id, reason
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "UPDATE payment_reminders SET status = 'cancelled' WHERE reference_type = 'purchase' AND reference_id = $1 AND status NOT IN ('settled','cancelled')",
        id
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    log_action(&state.pool, user.id, "purchase_cancelled", "purchase_order", Some(&id.to_string()),
        Some(json!({"po_number": po.po_number, "reason": reason}))).await;
    Ok(Json(json!({"message": format!("Purchase order {} cancelled", po.po_number)})))
}

pub async fn receive_goods(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
    Json(body): Json<Value>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "purchases.receive")?;
    let items = body.get("items").and_then(|v| v.as_array()).cloned().unwrap_or_default();

    let mut tx = state.pool.begin().await?;
    let po = sqlx::query!("SELECT po_number, status FROM purchase_orders WHERE id = $1 FOR UPDATE", id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound("Purchase order not found".into()))?;

    match po.status.as_str() {
        "approved" | "ordered" | "partially_received" => {}
        "draft" => return Err(AppError::BusinessLogic("Approve the purchase order before receiving goods".into())),
        other => return Err(AppError::BusinessLogic(format!("Cannot receive goods on a {} order", other.replace('_', " ")))),
    }

    let location_id = stock::resolve_location(&mut tx, uuid(body.get("location_id"))).await?;
    let reference = id.to_string();
    let mut received_lines = 0;

    for item in &items {
        let Some(product_id) = uuid(item.get("product_id")) else { continue };
        let qty = dec(item.get("received_quantity").or_else(|| item.get("quantity"))).unwrap_or(Decimal::ZERO);
        if qty <= Decimal::ZERO {
            continue;
        }

        let line = sqlx::query!(
            r#"SELECT poi.id, poi.quantity, poi.received_quantity, poi.taxable_value, p.name
               FROM purchase_order_items poi JOIN products p ON p.id = poi.product_id
               WHERE poi.purchase_order_id = $1 AND poi.product_id = $2
               ORDER BY poi.quantity - poi.received_quantity DESC LIMIT 1 FOR UPDATE OF poi"#,
            id, product_id
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::Validation("A received product is not on this purchase order".into()))?;

        let pending = line.quantity - line.received_quantity;
        if qty > pending {
            return Err(AppError::BusinessLogic(format!(
                "{}: receiving {} but only {} pending",
                line.name, qty.normalize(), pending.normalize()
            )));
        }

        stock::add_stock(&mut tx, product_id, location_id, qty, "purchase_receipt", Some(&reference), user.id, Some(&po.po_number)).await?;

        sqlx::query!("UPDATE purchase_order_items SET received_quantity = received_quantity + $1 WHERE id = $2", qty, line.id)
            .execute(&mut *tx)
            .await?;

        // Keep the product's cost price current with the latest effective purchase cost.
        if line.quantity > Decimal::ZERO {
            sqlx::query!(
                "UPDATE products SET purchase_price = $1, updated_at = NOW() WHERE id = $2",
                money(line.taxable_value / line.quantity), product_id
            )
            .execute(&mut *tx)
            .await?;
        }
        received_lines += 1;
    }

    if received_lines == 0 {
        return Err(AppError::Validation("Enter a received quantity for at least one item".into()));
    }

    let fully_received: bool = sqlx::query_scalar!(
        r#"SELECT NOT EXISTS(SELECT 1 FROM purchase_order_items WHERE purchase_order_id = $1 AND received_quantity < quantity) AS "done!""#,
        id
    )
    .fetch_one(&mut *tx)
    .await?;
    let new_status = if fully_received { "received" } else { "partially_received" };
    sqlx::query!("UPDATE purchase_orders SET status = $1, updated_at = NOW() WHERE id = $2", new_status, id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    log_action(&state.pool, user.id, "goods_received", "purchase_order", Some(&reference),
        Some(json!({"po_number": po.po_number, "lines": received_lines, "status": new_status}))).await;

    Ok(Json(json!({"message": "Goods received and stock updated", "status": new_status})))
}

pub async fn record_supplier_payment(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
    Json(body): Json<Value>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "payments.record")?;
    let amount = money(dec_field(&body, "amount")?.ok_or_else(|| AppError::Validation("Amount is required".into()))?);
    if amount <= Decimal::ZERO {
        return Err(AppError::Validation("Amount must be greater than zero".into()));
    }
    let method = payment_method(body.get("payment_method"))?;

    let mut tx = state.pool.begin().await?;
    let po = sqlx::query!(
        "SELECT po_number, supplier_id, status, advance_paid, outstanding_amount FROM purchase_orders WHERE id = $1 FOR UPDATE",
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Purchase order not found".into()))?;

    if po.status == "cancelled" {
        return Err(AppError::BusinessLogic("Cannot pay a cancelled purchase order".into()));
    }
    if amount > po.outstanding_amount {
        return Err(AppError::BusinessLogic(format!("Amount {} exceeds the outstanding {}", amount, po.outstanding_amount)));
    }

    let new_outstanding = po.outstanding_amount - amount;
    let new_paid = po.advance_paid + amount;
    let status = payment_status_for(new_paid, new_outstanding);

    sqlx::query!(
        "UPDATE purchase_orders SET advance_paid = $1, outstanding_amount = $2, payment_status = $3, updated_at = NOW() WHERE id = $4",
        new_paid, new_outstanding, status, id
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        r#"INSERT INTO payments (payment_date, amount, payment_method, direction, reference_type, reference_id, supplier_id,
               status, recorded_by, transaction_reference, notes)
           VALUES ($1,$2,$3,'outbound','purchase',$4,$5,'verified',$6,$7,$8)"#,
        today(), amount, method, id, po.supplier_id, user.id, text(body.get("transaction_reference")), text(body.get("notes"))
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        "UPDATE suppliers SET outstanding_balance = GREATEST(outstanding_balance - $1, 0) WHERE id = $2",
        amount, po.supplier_id
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        r#"UPDATE payment_reminders SET
               status = CASE WHEN $2::numeric <= 0 THEN 'settled' ELSE status END,
               outstanding_amount = $2::numeric
           WHERE reference_type = 'purchase' AND reference_id = $1 AND status NOT IN ('settled','cancelled')"#,
        id, new_outstanding
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    log_action(&state.pool, user.id, "supplier_payment_recorded", "purchase_order", Some(&id.to_string()),
        Some(json!({"po_number": po.po_number, "amount": amount, "method": method}))).await;

    Ok(Json(json!({"message": "Payment recorded", "outstanding_amount": new_outstanding, "payment_status": status})))
}
