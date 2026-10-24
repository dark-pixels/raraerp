use axum::{extract::{State, Extension, Path, Query}, Json, response::Response, http::header};
use uuid::Uuid;
use rust_decimal::Decimal;
use crate::{auth::models::AuthenticatedUser, errors::{AppError, AppResult}, middleware::permission::require_permission, routes::AppState};

pub async fn list_invoices(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(q): Query<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "invoices.create")?;
    let rows = sqlx::query!(
        r#"SELECT si.id, si.invoice_number, si.invoice_date, si.grand_total,
                  si.outstanding_amount, si.status, si.created_at,
                  c.name as "customer_name?"
           FROM sales_invoices si
           LEFT JOIN customers c ON c.id = si.customer_id
           ORDER BY si.invoice_date DESC LIMIT 100"#
    ).fetch_all(&state.pool).await?;

    let data: Vec<_> = rows.iter().map(|r| serde_json::json!({
        "id": r.id, "invoice_number": r.invoice_number, "customer_name": r.customer_name,
        "date": r.invoice_date, "total": r.grand_total,
        "outstanding": r.outstanding_amount, "status": r.status
    })).collect();
    Ok(Json(serde_json::json!({"data": data})))
}

pub async fn get_invoice(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "invoices.create")?;
    let row = sqlx::query!(
        r#"SELECT si.*, c.name as "customer_name?", c.gstin as "customer_gstin?", c.billing_address as "billing_address?"
           FROM sales_invoices si LEFT JOIN customers c ON c.id = si.customer_id WHERE si.id = $1"#, id
    ).fetch_optional(&state.pool).await?
     .ok_or_else(|| AppError::NotFound("Invoice not found".into()))?;

    let items = sqlx::query!(
        r#"SELECT sii.*, p.name as product_name, p.sku, p.hsn_code
           FROM sales_invoice_items sii JOIN products p ON p.id = sii.product_id
           WHERE sii.invoice_id = $1"#, id
    ).fetch_all(&state.pool).await?;

    let item_data: Vec<_> = items.iter().map(|i| serde_json::json!({
        "id": i.id, "product_name": i.product_name, "sku": i.sku,
        "hsn_code": i.hsn_code, "quantity": i.quantity,
        "unit_price": i.unit_price, "discount_percent": i.discount_percent,
        "taxable_value": i.taxable_value, "gst_rate": i.gst_rate,
        "cgst_amount": i.cgst_amount, "sgst_amount": i.sgst_amount,
        "igst_amount": i.igst_amount, "total": i.total
    })).collect();

    Ok(Json(serde_json::json!({
        "id": row.id, "invoice_number": row.invoice_number,
        "customer_name": row.customer_name, "customer_gstin": row.customer_gstin,
        "billing_address": row.billing_address,
        "invoice_date": row.invoice_date, "due_date": row.due_date,
        "subtotal": row.subtotal, "discount_amount": row.discount_amount,
        "taxable_value": row.taxable_value,
        "cgst_amount": row.cgst_amount, "sgst_amount": row.sgst_amount, "igst_amount": row.igst_amount,
        "grand_total": row.grand_total, "amount_paid": row.amount_paid,
        "outstanding_amount": row.outstanding_amount,
        "status": row.status, "notes": row.notes, "items": item_data
    })))
}

pub async fn create_invoice(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "invoices.create")?;

    let items = body.get("items").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    if items.is_empty() { return Err(AppError::Validation("At least one item required".into())); }

    let mut tx = state.pool.begin().await?;

    // Calculate totals server-side (never trust client totals)
    let mut subtotal = Decimal::ZERO;
    let mut total_cgst = Decimal::ZERO;
    let mut total_sgst = Decimal::ZERO;
    let mut total_igst = Decimal::ZERO;
    let mut calc_items: Vec<(Uuid, Decimal, Decimal, Decimal, Decimal, Decimal, Decimal, Decimal, Decimal)> = Vec::new();

    let customer_gstin: Option<String> = if let Some(cid) = body.get("customer_id").and_then(|v| v.as_str()).and_then(|s| s.parse::<Uuid>().ok()) {
        sqlx::query_scalar!("SELECT gstin FROM customers WHERE id = $1", cid).fetch_optional(&state.pool).await?.flatten()
    } else { None };

    // Determine IGST vs CGST+SGST (simplified: if customer has GSTIN from different state, use IGST)
    let use_igst = false; // Can be made dynamic based on place of supply

    for item in &items {
        let product_id: Uuid = item.get("product_id").and_then(|v| v.as_str()).and_then(|s| s.parse().ok())
            .ok_or_else(|| AppError::Validation("item.product_id required".into()))?;
        let qty: Decimal = item.get("quantity").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(Decimal::ONE);
        let unit_price: Decimal = item.get("unit_price").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(Decimal::ZERO);
        let discount: Decimal = item.get("discount_percent").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(Decimal::ZERO);

        // Get GST rate from product
        let gst_rate: Decimal = sqlx::query_scalar!("SELECT gst_rate FROM products WHERE id = $1", product_id)
            .fetch_optional(&mut *tx).await?.unwrap_or(Decimal::from(18));

        let discount_multiplier = Decimal::ONE - discount / Decimal::from(100);
        let taxable = qty * unit_price * discount_multiplier;
        let gst_amount = taxable * gst_rate / Decimal::from(100);
        let (cgst, sgst, igst) = if use_igst {
            (Decimal::ZERO, Decimal::ZERO, gst_amount)
        } else {
            (gst_amount / Decimal::from(2), gst_amount / Decimal::from(2), Decimal::ZERO)
        };
        let line_total = taxable + gst_amount;

        subtotal += taxable;
        total_cgst += cgst;
        total_sgst += sgst;
        total_igst += igst;
        calc_items.push((product_id, qty, unit_price, discount, taxable, gst_rate, cgst, sgst, igst));
    }

    let grand_total = subtotal + total_cgst + total_sgst + total_igst;
    let amount_paid: Decimal = body.get("amount_paid").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(Decimal::ZERO);
    let outstanding = grand_total - amount_paid;

    let invoice_number = generate_invoice_number(&mut tx).await?;
    let customer_id: Option<Uuid> = body.get("customer_id").and_then(|v| v.as_str()).and_then(|s| s.parse().ok());

    let status = if outstanding <= Decimal::ZERO { "paid" }
                 else if amount_paid > Decimal::ZERO { "partially_paid" }
                 else { "issued" };

    let id = sqlx::query_scalar!(
        r#"INSERT INTO sales_invoices (
            invoice_number, customer_id, invoice_date, subtotal, discount_amount,
            taxable_value, cgst_amount, sgst_amount, igst_amount, grand_total,
            amount_paid, outstanding_amount, status, created_by, notes
        ) VALUES ($1,$2,CURRENT_DATE,$3,0,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
        RETURNING id"#,
        invoice_number, customer_id, subtotal, total_cgst, total_sgst, total_igst,
        grand_total, amount_paid, outstanding, status, user.id,
        body.get("notes").and_then(|v| v.as_str())
    ).fetch_one(&mut *tx).await?;

    // Insert items
    for (i, (product_id, qty, unit_price, discount, taxable, gst_rate, cgst, sgst, igst)) in calc_items.iter().enumerate() {
        let total = taxable + cgst + sgst + igst;
        sqlx::query!(
            r#"INSERT INTO sales_invoice_items (invoice_id, product_id, quantity, unit_price, discount_percent, taxable_value, gst_rate, cgst_amount, sgst_amount, igst_amount, total)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)"#,
            id, product_id, qty, unit_price, discount, taxable, gst_rate, cgst, sgst, igst, total
        ).execute(&mut *tx).await?;

        // Decrease inventory
        sqlx::query!(
            r#"UPDATE inventory_balances SET
               on_hand = on_hand - $1
               WHERE product_id = $2 AND on_hand >= $1"#,
            qty, product_id
        ).execute(&mut *tx).await?;

        // Record stock movement
        sqlx::query!(
            r#"INSERT INTO stock_movements (product_id, quantity, movement_type, reference_document, user_id)
               VALUES ($1, $2, 'sales_dispatch', $3, $4)"#,
            product_id, qty, id.to_string(), user.id
        ).execute(&mut *tx).await?;
    }

    // Record initial payment if any
    if amount_paid > Decimal::ZERO {
        let method = body.get("payment_method").and_then(|v| v.as_str()).unwrap_or("cash");
        sqlx::query!(
            r#"INSERT INTO payments (amount, payment_method, direction, reference_type, reference_id, customer_id, status, recorded_by)
               VALUES ($1,$2,'inbound','invoice',$3,$4,'verified',$5)"#,
            amount_paid, method, id, customer_id, user.id
        ).execute(&mut *tx).await?;

        // Update customer outstanding
        if let Some(cid) = customer_id {
            sqlx::query!(
                "UPDATE customers SET outstanding_balance = outstanding_balance + $1 WHERE id = $2",
                outstanding, cid
            ).execute(&mut *tx).await?;
        }
    }

    // Schedule payment reminder if credit sale
    if outstanding > Decimal::ZERO {
        if let Some(cid) = customer_id {
            let credit_days: i32 = sqlx::query_scalar!("SELECT credit_period_days FROM customers WHERE id = $1", cid)
                .fetch_optional(&mut *tx).await?.unwrap_or(0);
            if credit_days > 0 {
                sqlx::query!(
                    r#"UPDATE sales_invoices SET due_date = CURRENT_DATE + $1::integer * INTERVAL '1 day' WHERE id = $2"#,
                    credit_days, id
                ).execute(&mut *tx).await?;
            }
        }
    }

    tx.commit().await?;

    Ok(Json(serde_json::json!({
        "id": id,
        "invoice_number": invoice_number,
        "grand_total": grand_total,
        "outstanding_amount": outstanding,
        "status": status,
        "message": "Invoice created successfully"
    })))
}

pub async fn cancel_invoice(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "invoices.cancel")?;
    let reason = body.get("reason").and_then(|v| v.as_str()).ok_or_else(|| AppError::Validation("reason required".into()))?;

    let existing = sqlx::query_scalar!("SELECT status FROM sales_invoices WHERE id = $1", id)
        .fetch_optional(&state.pool).await?
        .ok_or_else(|| AppError::NotFound("Invoice not found".into()))?;

    if existing == "paid" || existing == "cancelled" {
        return Err(AppError::BusinessLogic(format!("Cannot cancel a {} invoice", existing)));
    }

    sqlx::query!(
        "UPDATE sales_invoices SET status = 'cancelled', updated_at = NOW() WHERE id = $1", id
    ).execute(&state.pool).await?;

    // Log to audit
    sqlx::query!(
        r#"INSERT INTO audit_logs (user_id, action, resource_type, resource_id, changes)
           VALUES ($1, 'invoice_cancelled', 'sales_invoice', $2, $3)"#,
        user.id, id.to_string(), serde_json::json!({"reason": reason})
    ).execute(&state.pool).await.ok();

    Ok(Json(serde_json::json!({"message": "Invoice cancelled"})))
}

pub async fn record_payment(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "payments.record")?;

    let amount: Decimal = body.get("amount").and_then(|v| v.as_str()).and_then(|s| s.parse().ok())
        .ok_or_else(|| AppError::Validation("amount required".into()))?;
    let method = body.get("payment_method").and_then(|v| v.as_str()).unwrap_or("cash");

    let invoice = sqlx::query!(
        "SELECT customer_id, outstanding_amount, status FROM sales_invoices WHERE id = $1", id
    ).fetch_optional(&state.pool).await?
     .ok_or_else(|| AppError::NotFound("Invoice not found".into()))?;

    if invoice.status == "paid" || invoice.status == "cancelled" {
        return Err(AppError::BusinessLogic("Invoice is already paid or cancelled".into()));
    }

    if amount > invoice.outstanding_amount {
        return Err(AppError::BusinessLogic(format!("Amount {} exceeds outstanding {}", amount, invoice.outstanding_amount)));
    }

    let mut tx = state.pool.begin().await?;

    let new_outstanding = invoice.outstanding_amount - amount;
    let new_status = if new_outstanding <= Decimal::ZERO { "paid" } else { "partially_paid" };

    sqlx::query!(
        "UPDATE sales_invoices SET amount_paid = amount_paid + $1, outstanding_amount = $2, status = $3, updated_at = NOW() WHERE id = $4",
        amount, new_outstanding, new_status, id
    ).execute(&mut *tx).await?;

    sqlx::query!(
        r#"INSERT INTO payments (amount, payment_method, direction, reference_type, reference_id, customer_id, status, recorded_by, transaction_reference)
           VALUES ($1,$2,'inbound','invoice',$3,$4,'verified',$5,$6)"#,
        amount, method, id, invoice.customer_id, user.id,
        body.get("transaction_reference").and_then(|v| v.as_str())
    ).execute(&mut *tx).await?;

    if let Some(cid) = invoice.customer_id {
        sqlx::query!(
            "UPDATE customers SET outstanding_balance = GREATEST(outstanding_balance - $1, 0) WHERE id = $2",
            amount, cid
        ).execute(&mut *tx).await?;
    }

    tx.commit().await?;

    Ok(Json(serde_json::json!({
        "message": "Payment recorded",
        "new_outstanding": new_outstanding,
        "status": new_status
    })))
}

pub async fn create_credit_note(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "invoices.cancel")?;
    let amount: Decimal = body.get("amount").and_then(|v| v.as_str()).and_then(|s| s.parse().ok())
        .ok_or_else(|| AppError::Validation("amount required".into()))?;
    let note_id = sqlx::query_scalar!(
        "INSERT INTO credit_notes (original_invoice_id, total, reason, status, created_by) VALUES ($1,$2,$3,'issued',$4) RETURNING id",
        id, amount,
        body.get("reason").and_then(|v| v.as_str()).unwrap_or("Return"),
        user.id
    ).fetch_one(&state.pool).await?;
    Ok(Json(serde_json::json!({"credit_note_id": note_id})))
}

pub async fn download_pdf(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Response> {
    use axum::body::Body;
    require_permission(&user, "invoices.create")?;

    let row = sqlx::query!(
        r#"SELECT si.*, c.name as "customer_name?" FROM sales_invoices si
           LEFT JOIN customers c ON c.id = si.customer_id WHERE si.id = $1"#, id
    ).fetch_optional(&state.pool).await?
     .ok_or_else(|| AppError::NotFound("Invoice not found".into()))?;

    // Generate simple PDF bytes using printpdf
    let pdf_bytes = crate::invoices::service::generate_invoice_pdf(&row.invoice_number, row.customer_name.as_deref(), row.grand_total).await?;

    Ok(Response::builder()
        .header(header::CONTENT_TYPE, "application/pdf")
        .header(header::CONTENT_DISPOSITION, format!("attachment; filename=\"invoice-{}.pdf\"", row.invoice_number))
        .body(Body::from(pdf_bytes))
        .unwrap())
}

async fn generate_invoice_number(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) -> AppResult<String> {
    let count: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM sales_invoices")
        .fetch_one(&mut **tx).await?.unwrap_or(0);
    let year = chrono::Utc::now().format("%Y").to_string();
    Ok(format!("INV-{}-{:06}", year, count + 1))
}
