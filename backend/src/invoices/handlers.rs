use axum::{
    extract::{Extension, Path, Query, State},
    http::header,
    response::Response,
    Json,
};
use rust_decimal::Decimal;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    audit::service::log_action,
    auth::models::AuthenticatedUser,
    common::{dec, dec_field, money, next_document_number, paginated, payment_method, text, today, uuid, ListQuery},
    errors::{AppError, AppResult},
    inventory::stock,
    middleware::permission::require_permission,
    routes::AppState,
    settings::load_company,
};

pub async fn list_invoices(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "invoices.create")?;
    let search = q.search();
    let status = q.status();

    let rows = sqlx::query!(
        r#"SELECT si.id, si.invoice_number, si.invoice_date, si.due_date, si.customer_id,
                  si.taxable_value, si.cgst_amount, si.sgst_amount, si.igst_amount,
                  si.grand_total, si.amount_paid, si.outstanding_amount, si.status,
                  si.payment_method, si.created_at,
                  c.name AS "customer_name?", c.phone AS "customer_phone?"
           FROM sales_invoices si
           LEFT JOIN customers c ON c.id = si.customer_id
           WHERE ($1::text IS NULL OR si.invoice_number ILIKE '%' || $1 || '%'
                  OR c.name ILIKE '%' || $1 || '%' OR c.phone ILIKE '%' || $1 || '%')
             AND ($2::text IS NULL OR si.status = $2)
             AND ($3::uuid IS NULL OR si.customer_id = $3)
             AND ($4::date IS NULL OR si.invoice_date >= $4)
             AND ($5::date IS NULL OR si.invoice_date <= $5)
           ORDER BY si.invoice_date DESC, si.created_at DESC
           LIMIT $6 OFFSET $7"#,
        search, status, q.customer_id, q.from, q.to, q.per_page(), q.offset()
    )
    .fetch_all(&state.pool)
    .await?;

    let total: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM sales_invoices si
           LEFT JOIN customers c ON c.id = si.customer_id
           WHERE ($1::text IS NULL OR si.invoice_number ILIKE '%' || $1 || '%'
                  OR c.name ILIKE '%' || $1 || '%' OR c.phone ILIKE '%' || $1 || '%')
             AND ($2::text IS NULL OR si.status = $2)
             AND ($3::uuid IS NULL OR si.customer_id = $3)
             AND ($4::date IS NULL OR si.invoice_date >= $4)
             AND ($5::date IS NULL OR si.invoice_date <= $5)"#,
        search, status, q.customer_id, q.from, q.to
    )
    .fetch_one(&state.pool)
    .await?;

    let data = rows
        .into_iter()
        .map(|r| {
            json!({
                "id": r.id, "invoice_number": r.invoice_number,
                "invoice_date": r.invoice_date, "date": r.invoice_date, "due_date": r.due_date,
                "customer_id": r.customer_id, "customer_name": r.customer_name, "customer_phone": r.customer_phone,
                "taxable_value": r.taxable_value,
                "tax_amount": r.cgst_amount + r.sgst_amount + r.igst_amount,
                "grand_total": r.grand_total, "total": r.grand_total,
                "amount_paid": r.amount_paid,
                "outstanding_amount": r.outstanding_amount, "outstanding": r.outstanding_amount,
                "status": r.status, "payment_method": r.payment_method, "created_at": r.created_at,
            })
        })
        .collect();

    Ok(Json(paginated(data, total, &q)))
}

pub async fn get_invoice(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "invoices.create")?;
    let row = sqlx::query!(
        r#"SELECT si.*, c.name AS "customer_name?", c.phone AS "customer_phone?",
                  c.gstin AS "customer_gstin?", c.billing_address AS "billing_address?",
                  u.full_name AS "created_by_name?"
           FROM sales_invoices si
           LEFT JOIN customers c ON c.id = si.customer_id
           LEFT JOIN users u ON u.id = si.created_by
           WHERE si.id = $1"#,
        id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Invoice not found".into()))?;

    let items = sqlx::query!(
        r#"SELECT sii.*, p.name AS product_name, p.sku, p.hsn_code
           FROM sales_invoice_items sii JOIN products p ON p.id = sii.product_id
           WHERE sii.invoice_id = $1 ORDER BY p.name"#,
        id
    )
    .fetch_all(&state.pool)
    .await?;

    let payments = sqlx::query!(
        r#"SELECT id, payment_date, amount, payment_method, status, transaction_reference, created_at
           FROM payments WHERE reference_type = 'invoice' AND reference_id = $1 ORDER BY created_at"#,
        id
    )
    .fetch_all(&state.pool)
    .await?;

    let credit_notes = sqlx::query!(
        "SELECT id, total, reason, status, created_at FROM credit_notes WHERE original_invoice_id = $1 ORDER BY created_at",
        id
    )
    .fetch_all(&state.pool)
    .await?;

    let item_data: Vec<Value> = items
        .iter()
        .map(|i| {
            json!({
                "id": i.id, "product_id": i.product_id, "product_name": i.product_name, "sku": i.sku,
                "hsn_code": i.hsn_code, "quantity": i.quantity,
                "unit_price": i.unit_price, "discount_percent": i.discount_percent,
                "taxable_value": i.taxable_value, "gst_rate": i.gst_rate,
                "cgst_amount": i.cgst_amount, "sgst_amount": i.sgst_amount,
                "igst_amount": i.igst_amount, "total": i.total
            })
        })
        .collect();

    Ok(Json(json!({
        "id": row.id, "invoice_number": row.invoice_number,
        "customer_id": row.customer_id, "customer_name": row.customer_name,
        "customer_phone": row.customer_phone, "customer_gstin": row.customer_gstin,
        "billing_address": row.billing_address,
        "invoice_date": row.invoice_date, "due_date": row.due_date,
        "subtotal": row.subtotal, "discount_amount": row.discount_amount,
        "taxable_value": row.taxable_value,
        "cgst_amount": row.cgst_amount, "sgst_amount": row.sgst_amount, "igst_amount": row.igst_amount,
        "grand_total": row.grand_total, "amount_paid": row.amount_paid,
        "outstanding_amount": row.outstanding_amount,
        "status": row.status, "payment_method": row.payment_method, "notes": row.notes,
        "created_by_name": row.created_by_name, "created_at": row.created_at,
        "items": item_data,
        "payments": payments.iter().map(|p| json!({
            "id": p.id, "payment_date": p.payment_date, "amount": p.amount,
            "payment_method": p.payment_method, "status": p.status,
            "transaction_reference": p.transaction_reference, "created_at": p.created_at
        })).collect::<Vec<_>>(),
        "credit_notes": credit_notes.iter().map(|c| json!({
            "id": c.id, "total": c.total, "reason": c.reason, "status": c.status, "created_at": c.created_at
        })).collect::<Vec<_>>(),
    })))
}

struct LineCalc {
    product_id: Uuid,
    product_name: String,
    qty: Decimal,
    unit_price: Decimal,
    discount: Decimal,
    taxable: Decimal,
    gst_rate: Decimal,
    cgst: Decimal,
    sgst: Decimal,
    igst: Decimal,
}

pub async fn create_invoice(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(body): Json<Value>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "invoices.create")?;

    let items = body.get("items").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    if items.is_empty() {
        return Err(AppError::Validation("Add at least one item to the invoice".into()));
    }
    if items.len() > 500 {
        return Err(AppError::Validation("An invoice can have at most 500 lines".into()));
    }

    let method = payment_method(body.get("payment_method"))?;
    let amount_paid = dec_field(&body, "amount_paid")?.unwrap_or(Decimal::ZERO);
    if amount_paid < Decimal::ZERO {
        return Err(AppError::Validation("Amount paid cannot be negative".into()));
    }

    let company = load_company(&state.pool).await?;
    let mut tx = state.pool.begin().await?;

    let customer = match uuid(body.get("customer_id")) {
        Some(cid) => Some(
            sqlx::query!(
                r#"SELECT id, name, gstin, customer_type, credit_limit, credit_period_days,
                          outstanding_balance, is_active
                   FROM customers WHERE id = $1 FOR UPDATE"#,
                cid
            )
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::Validation("Selected customer was not found".into()))?,
        ),
        None => None,
    };
    if let Some(c) = &customer {
        if !c.is_active {
            return Err(AppError::BusinessLogic(format!("Customer {} is inactive", c.name)));
        }
    }

    // Inter-state supply (different GST state codes) attracts IGST; otherwise CGST + SGST.
    let customer_state = customer.as_ref().and_then(|c| c.gstin.as_deref()).filter(|g| g.len() >= 2).map(|g| &g[..2]);
    let use_igst = matches!((company.state_code(), customer_state), (Some(a), Some(b)) if a != b);
    let wholesale = customer.as_ref().map(|c| c.customer_type == "wholesale").unwrap_or(false);

    let hundred = Decimal::from(100);
    let mut lines: Vec<LineCalc> = Vec::with_capacity(items.len());
    for (idx, item) in items.iter().enumerate() {
        let line_no = idx + 1;
        let product_id = uuid(item.get("product_id"))
            .ok_or_else(|| AppError::Validation(format!("Line {}: product is required", line_no)))?;
        let product = sqlx::query!(
            "SELECT name, status, gst_rate, retail_price, wholesale_price, min_selling_price FROM products WHERE id = $1",
            product_id
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::Validation(format!("Line {}: product not found", line_no)))?;

        if product.status != "active" {
            return Err(AppError::BusinessLogic(format!("{} is {} and cannot be sold", product.name, product.status)));
        }

        let qty = dec(item.get("quantity")).unwrap_or(Decimal::ONE);
        if qty <= Decimal::ZERO {
            return Err(AppError::Validation(format!("{}: quantity must be greater than zero", product.name)));
        }
        let default_price = if wholesale && product.wholesale_price > Decimal::ZERO { product.wholesale_price } else { product.retail_price };
        let unit_price = dec(item.get("unit_price")).unwrap_or(default_price);
        if unit_price < Decimal::ZERO {
            return Err(AppError::Validation(format!("{}: price cannot be negative", product.name)));
        }
        let discount = dec(item.get("discount_percent")).unwrap_or(Decimal::ZERO);
        if discount < Decimal::ZERO || discount > hundred {
            return Err(AppError::Validation(format!("{}: discount must be between 0 and 100%", product.name)));
        }

        let gross = money(qty * unit_price);
        let taxable = gross - money(gross * discount / hundred);
        if let Some(min_price) = product.min_selling_price {
            if min_price > Decimal::ZERO && taxable < money(min_price * qty) {
                return Err(AppError::BusinessLogic(format!(
                    "{}: price after discount is below the minimum selling price of {}",
                    product.name, min_price
                )));
            }
        }
        let gst = money(taxable * product.gst_rate / hundred);
        let (cgst, sgst, igst) = if use_igst {
            (Decimal::ZERO, Decimal::ZERO, gst)
        } else {
            let half = money(gst / Decimal::from(2));
            (half, gst - half, Decimal::ZERO)
        };

        lines.push(LineCalc {
            product_id, product_name: product.name, qty, unit_price, discount,
            taxable, gst_rate: product.gst_rate, cgst, sgst, igst,
        });
    }

    let subtotal: Decimal = lines.iter().map(|l| money(l.qty * l.unit_price)).sum();
    let taxable_total: Decimal = lines.iter().map(|l| l.taxable).sum();
    let discount_total = subtotal - taxable_total;
    let cgst_total: Decimal = lines.iter().map(|l| l.cgst).sum();
    let sgst_total: Decimal = lines.iter().map(|l| l.sgst).sum();
    let igst_total: Decimal = lines.iter().map(|l| l.igst).sum();
    let grand_total = taxable_total + cgst_total + sgst_total + igst_total;

    // Any amount tendered beyond the total is change, not a payment.
    let applied = amount_paid.min(grand_total);
    let change_due = amount_paid - applied;
    let outstanding = grand_total - applied;

    if outstanding > Decimal::ZERO {
        let Some(c) = &customer else {
            return Err(AppError::BusinessLogic(
                "Walk-in sales must be paid in full. Select a customer to sell on credit.".into(),
            ));
        };
        if c.credit_limit > Decimal::ZERO && c.outstanding_balance + outstanding > c.credit_limit {
            return Err(AppError::BusinessLogic(format!(
                "This sale would take {} over their credit limit of {} (current balance {})",
                c.name, c.credit_limit, c.outstanding_balance
            )));
        }
    }

    let status = if outstanding <= Decimal::ZERO {
        "paid"
    } else if applied > Decimal::ZERO {
        "partially_paid"
    } else {
        "issued"
    };

    let invoice_date = today();
    let due_date = if outstanding > Decimal::ZERO {
        let days = customer.as_ref().map(|c| c.credit_period_days).unwrap_or(0).max(0);
        Some(invoice_date + chrono::Duration::days(days as i64))
    } else {
        None
    };

    let prefix = format!("{}-{}", company.invoice_prefix, invoice_date.format("%Y"));
    let invoice_number = next_document_number(&mut tx, "sales_invoices", "invoice_number", &prefix).await?;
    let customer_id = customer.as_ref().map(|c| c.id);

    let id = sqlx::query_scalar!(
        r#"INSERT INTO sales_invoices (
            invoice_number, customer_id, invoice_date, due_date, subtotal, discount_amount,
            taxable_value, cgst_amount, sgst_amount, igst_amount, grand_total,
            amount_paid, outstanding_amount, status, payment_method, created_by, notes
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17)
        RETURNING id"#,
        invoice_number, customer_id, invoice_date, due_date, subtotal, discount_total,
        taxable_total, cgst_total, sgst_total, igst_total, grand_total,
        applied, outstanding, status, method, user.id, text(body.get("notes"))
    )
    .fetch_one(&mut *tx)
    .await?;

    let reference = id.to_string();
    for l in &lines {
        sqlx::query!(
            r#"INSERT INTO sales_invoice_items (invoice_id, product_id, quantity, unit_price, discount_percent,
                   taxable_value, gst_rate, cgst_amount, sgst_amount, igst_amount, total)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)"#,
            id, l.product_id, l.qty, l.unit_price, l.discount, l.taxable, l.gst_rate,
            l.cgst, l.sgst, l.igst, l.taxable + l.cgst + l.sgst + l.igst
        )
        .execute(&mut *tx)
        .await?;

        stock::dispatch_stock(&mut tx, l.product_id, &l.product_name, l.qty, &reference, user.id).await?;
    }

    if applied > Decimal::ZERO {
        sqlx::query!(
            r#"INSERT INTO payments (payment_date, amount, payment_method, direction, reference_type, reference_id,
                   customer_id, status, recorded_by, transaction_reference)
               VALUES ($1,$2,$3,'inbound','invoice',$4,$5,'verified',$6,$7)"#,
            invoice_date, applied, method, id, customer_id, user.id, text(body.get("transaction_reference"))
        )
        .execute(&mut *tx)
        .await?;
    }

    if let (Some(cid), true) = (customer_id, outstanding > Decimal::ZERO) {
        sqlx::query!(
            "UPDATE customers SET outstanding_balance = outstanding_balance + $1 WHERE id = $2",
            outstanding, cid
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query!(
            r#"INSERT INTO payment_reminders (reference_type, reference_id, due_date, outstanding_amount, status, reminder_days_before)
               VALUES ('invoice', $1, $2, $3, 'scheduled', 2)"#,
            id, due_date, outstanding
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    log_action(&state.pool, user.id, "invoice_created", "sales_invoice", Some(&reference), Some(json!({
        "invoice_number": invoice_number, "grand_total": grand_total, "amount_paid": applied
    }))).await;

    Ok(Json(json!({
        "id": id,
        "invoice_number": invoice_number,
        "grand_total": grand_total,
        "amount_paid": applied,
        "outstanding_amount": outstanding,
        "change_due": change_due,
        "status": status,
        "message": "Invoice created successfully"
    })))
}

pub async fn cancel_invoice(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
    Json(body): Json<Value>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "invoices.cancel")?;
    let reason = text(body.get("reason")).ok_or_else(|| AppError::Validation("A cancellation reason is required".into()))?;

    let mut tx = state.pool.begin().await?;
    let inv = sqlx::query!(
        "SELECT invoice_number, status, customer_id, amount_paid, outstanding_amount FROM sales_invoices WHERE id = $1 FOR UPDATE",
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Invoice not found".into()))?;

    if inv.status == "cancelled" {
        return Err(AppError::BusinessLogic("Invoice is already cancelled".into()));
    }
    if inv.amount_paid > Decimal::ZERO {
        return Err(AppError::BusinessLogic(
            "This invoice has payments recorded. Issue a credit note instead of cancelling.".into(),
        ));
    }

    stock::reverse_dispatch(&mut tx, &id.to_string(), user.id, &format!("Invoice {} cancelled", inv.invoice_number)).await?;

    if let Some(cid) = inv.customer_id {
        sqlx::query!(
            "UPDATE customers SET outstanding_balance = GREATEST(outstanding_balance - $1, 0) WHERE id = $2",
            inv.outstanding_amount, cid
        )
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query!(
        r#"UPDATE sales_invoices SET status = 'cancelled', outstanding_amount = 0, updated_at = NOW(),
               notes = CONCAT_WS(E'\n', NULLIF(notes, ''), 'Cancelled: ' || $2::text)
           WHERE id = $1"#,
        id, reason
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        "UPDATE payment_reminders SET status = 'cancelled' WHERE reference_type = 'invoice' AND reference_id = $1 AND status NOT IN ('settled','cancelled')",
        id
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    log_action(&state.pool, user.id, "invoice_cancelled", "sales_invoice", Some(&id.to_string()),
        Some(json!({"invoice_number": inv.invoice_number, "reason": reason}))).await;

    Ok(Json(json!({"message": format!("Invoice {} cancelled and stock returned", inv.invoice_number)})))
}

/// Brings an invoice's reminder in line with its new outstanding amount.
async fn sync_invoice_reminder(tx: &mut stock::Tx<'_>, invoice_id: Uuid, outstanding: Decimal) -> AppResult<()> {
    if outstanding <= Decimal::ZERO {
        sqlx::query!(
            "UPDATE payment_reminders SET status = 'settled', outstanding_amount = 0 WHERE reference_type = 'invoice' AND reference_id = $1 AND status NOT IN ('settled','cancelled')",
            invoice_id
        )
        .execute(&mut **tx)
        .await?;
    } else {
        sqlx::query!(
            "UPDATE payment_reminders SET outstanding_amount = $2 WHERE reference_type = 'invoice' AND reference_id = $1 AND status NOT IN ('settled','cancelled')",
            invoice_id, outstanding
        )
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

pub async fn record_payment(
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
    let invoice = sqlx::query!(
        "SELECT invoice_number, customer_id, outstanding_amount, status FROM sales_invoices WHERE id = $1 FOR UPDATE",
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Invoice not found".into()))?;

    if invoice.status == "cancelled" {
        return Err(AppError::BusinessLogic("Cannot record a payment against a cancelled invoice".into()));
    }
    if invoice.outstanding_amount <= Decimal::ZERO {
        return Err(AppError::BusinessLogic("This invoice is already fully paid".into()));
    }
    if amount > invoice.outstanding_amount {
        return Err(AppError::BusinessLogic(format!(
            "Amount {} exceeds the outstanding balance of {}",
            amount, invoice.outstanding_amount
        )));
    }

    let new_outstanding = invoice.outstanding_amount - amount;
    let new_status = if new_outstanding <= Decimal::ZERO { "paid" } else { "partially_paid" };

    sqlx::query!(
        "UPDATE sales_invoices SET amount_paid = amount_paid + $1, outstanding_amount = $2, status = $3, updated_at = NOW() WHERE id = $4",
        amount, new_outstanding, new_status, id
    )
    .execute(&mut *tx)
    .await?;

    let payment_id = sqlx::query_scalar!(
        r#"INSERT INTO payments (payment_date, amount, payment_method, direction, reference_type, reference_id, customer_id,
               status, recorded_by, transaction_reference, notes)
           VALUES ($1,$2,$3,'inbound','invoice',$4,$5,'verified',$6,$7,$8) RETURNING id"#,
        today(), amount, method, id, invoice.customer_id, user.id,
        text(body.get("transaction_reference")), text(body.get("notes"))
    )
    .fetch_one(&mut *tx)
    .await?;

    if let Some(cid) = invoice.customer_id {
        sqlx::query!(
            "UPDATE customers SET outstanding_balance = GREATEST(outstanding_balance - $1, 0) WHERE id = $2",
            amount, cid
        )
        .execute(&mut *tx)
        .await?;
    }

    sync_invoice_reminder(&mut tx, id, new_outstanding).await?;
    tx.commit().await?;

    log_action(&state.pool, user.id, "payment_recorded", "sales_invoice", Some(&id.to_string()),
        Some(json!({"invoice_number": invoice.invoice_number, "amount": amount, "method": method}))).await;

    Ok(Json(json!({
        "message": "Payment recorded",
        "payment_id": payment_id,
        "new_outstanding": new_outstanding,
        "outstanding_amount": new_outstanding,
        "status": new_status
    })))
}

pub async fn create_credit_note(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
    Json(body): Json<Value>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "invoices.cancel")?;
    let amount = money(dec_field(&body, "amount")?.ok_or_else(|| AppError::Validation("Amount is required".into()))?);
    if amount <= Decimal::ZERO {
        return Err(AppError::Validation("Amount must be greater than zero".into()));
    }
    let reason = text(body.get("reason")).unwrap_or_else(|| "Return".into());

    let mut tx = state.pool.begin().await?;
    let inv = sqlx::query!(
        "SELECT invoice_number, status, customer_id, grand_total, amount_paid, outstanding_amount FROM sales_invoices WHERE id = $1 FOR UPDATE",
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Invoice not found".into()))?;

    if inv.status == "cancelled" {
        return Err(AppError::BusinessLogic("Cannot issue a credit note for a cancelled invoice".into()));
    }

    let already_credited: Decimal = sqlx::query_scalar!(
        r#"SELECT COALESCE(SUM(total), 0) AS "sum!" FROM credit_notes WHERE original_invoice_id = $1 AND status <> 'cancelled'"#,
        id
    )
    .fetch_one(&mut *tx)
    .await?;

    let creditable = inv.grand_total - already_credited;
    if amount > creditable {
        return Err(AppError::BusinessLogic(format!("Only {} remains creditable on this invoice", creditable)));
    }

    // The credit first reduces what the customer still owes; any remainder is a refund due.
    let reduce = amount.min(inv.outstanding_amount);
    let new_outstanding = inv.outstanding_amount - reduce;
    let fully_returned = already_credited + amount >= inv.grand_total;
    let new_status = if fully_returned {
        "returned".to_string()
    } else if new_outstanding <= Decimal::ZERO {
        "paid".to_string()
    } else {
        inv.status.clone()
    };

    sqlx::query!(
        "UPDATE sales_invoices SET outstanding_amount = $1, status = $2, updated_at = NOW() WHERE id = $3",
        new_outstanding, new_status, id
    )
    .execute(&mut *tx)
    .await?;

    if let (Some(cid), true) = (inv.customer_id, reduce > Decimal::ZERO) {
        sqlx::query!(
            "UPDATE customers SET outstanding_balance = GREATEST(outstanding_balance - $1, 0) WHERE id = $2",
            reduce, cid
        )
        .execute(&mut *tx)
        .await?;
    }

    // Optionally put returned goods back on the shelf.
    if let Some(restock) = body.get("restock_items").and_then(|v| v.as_array()) {
        let location = stock::resolve_location(&mut tx, uuid(body.get("location_id"))).await?;
        for item in restock {
            let (Some(pid), Some(qty)) = (uuid(item.get("product_id")), dec(item.get("quantity"))) else { continue };
            stock::add_stock(&mut tx, pid, location, qty, "customer_return", Some(&id.to_string()), user.id, Some(&reason)).await?;
        }
    }

    sync_invoice_reminder(&mut tx, id, new_outstanding).await?;

    let note_id = sqlx::query_scalar!(
        "INSERT INTO credit_notes (original_invoice_id, total, reason, status, created_by) VALUES ($1,$2,$3,'issued',$4) RETURNING id",
        id, amount, reason, user.id
    )
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    log_action(&state.pool, user.id, "credit_note_issued", "sales_invoice", Some(&id.to_string()),
        Some(json!({"invoice_number": inv.invoice_number, "amount": amount, "reason": reason}))).await;

    Ok(Json(json!({
        "credit_note_id": note_id,
        "outstanding_amount": new_outstanding,
        "refund_due": amount - reduce,
        "status": new_status,
        "message": "Credit note issued"
    })))
}

pub async fn download_pdf(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Response> {
    use axum::body::Body;
    require_permission(&user, "invoices.create")?;

    let row = sqlx::query!(
        r#"SELECT si.invoice_number, si.invoice_date, si.due_date, si.subtotal, si.discount_amount,
                  si.taxable_value, si.cgst_amount, si.sgst_amount, si.igst_amount, si.grand_total,
                  si.amount_paid, si.outstanding_amount, si.status, si.payment_method, si.notes,
                  c.name AS "customer_name?", c.phone AS "customer_phone?", c.gstin AS "customer_gstin?",
                  c.billing_address AS "billing_address?"
           FROM sales_invoices si LEFT JOIN customers c ON c.id = si.customer_id WHERE si.id = $1"#,
        id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Invoice not found".into()))?;

    let items = sqlx::query!(
        r#"SELECT p.name, p.hsn_code, sii.quantity, sii.unit_price, sii.discount_percent, sii.taxable_value,
                  sii.gst_rate, sii.cgst_amount, sii.sgst_amount, sii.igst_amount, sii.total
           FROM sales_invoice_items sii JOIN products p ON p.id = sii.product_id
           WHERE sii.invoice_id = $1 ORDER BY p.name"#,
        id
    )
    .fetch_all(&state.pool)
    .await?;

    let company = load_company(&state.pool).await?;

    let doc = crate::invoices::service::InvoiceDoc {
        company,
        invoice_number: row.invoice_number.clone(),
        invoice_date: row.invoice_date,
        due_date: row.due_date,
        status: row.status,
        payment_method: row.payment_method,
        customer_name: row.customer_name,
        customer_phone: row.customer_phone,
        customer_gstin: row.customer_gstin,
        billing_address: row.billing_address,
        subtotal: row.subtotal,
        discount_amount: row.discount_amount,
        taxable_value: row.taxable_value,
        cgst: row.cgst_amount,
        sgst: row.sgst_amount,
        igst: row.igst_amount,
        grand_total: row.grand_total,
        amount_paid: row.amount_paid,
        outstanding: row.outstanding_amount,
        notes: row.notes,
        lines: items
            .into_iter()
            .map(|i| crate::invoices::service::InvoiceLine {
                name: i.name,
                hsn: i.hsn_code,
                qty: i.quantity,
                rate: i.unit_price,
                discount: i.discount_percent,
                taxable: i.taxable_value,
                gst_rate: i.gst_rate,
                tax: i.cgst_amount + i.sgst_amount + i.igst_amount,
                total: i.total,
            })
            .collect(),
    };

    let pdf_bytes = crate::invoices::service::generate_invoice_pdf(&doc)?;

    Response::builder()
        .header(header::CONTENT_TYPE, "application/pdf")
        .header(
            header::CONTENT_DISPOSITION,
            format!("inline; filename=\"{}.pdf\"", row.invoice_number.replace(['"', '/', '\\'], "-")),
        )
        .body(Body::from(pdf_bytes))
        .map_err(|e| AppError::Internal(e.to_string()))
}
