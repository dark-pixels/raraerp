use axum::{
    extract::{Extension, Path, Query, State},
    Json,
};
use hmac::{Hmac, Mac};
use rust_decimal::{prelude::ToPrimitive, Decimal};
use serde_json::{json, Value};
use sha2::Sha256;
use uuid::Uuid;

use crate::{
    auth::models::AuthenticatedUser,
    common::{non_empty, paginated, today, uuid, ListQuery},
    errors::{AppError, AppResult},
    middleware::permission::require_permission,
    routes::AppState,
    settings::load_company,
};

pub async fn list_payments(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "payments.record")?;
    let search = q.search();
    let direction = non_empty(&q.direction);
    let method = non_empty(&q.method);

    let rows = sqlx::query!(
        r#"SELECT p.id, p.payment_date, p.amount, p.payment_method, p.direction, p.status,
                  p.reference_type, p.reference_id, p.transaction_reference, p.gateway_reference, p.notes, p.created_at,
                  c.name AS "customer_name?", s.business_name AS "supplier_name?",
                  COALESCE(si.invoice_number, po.po_number) AS "document_number?"
           FROM payments p
           LEFT JOIN customers c ON c.id = p.customer_id
           LEFT JOIN suppliers s ON s.id = p.supplier_id
           LEFT JOIN sales_invoices si ON p.reference_type = 'invoice' AND si.id = p.reference_id
           LEFT JOIN purchase_orders po ON p.reference_type = 'purchase' AND po.id = p.reference_id
           WHERE ($1::text IS NULL OR p.direction = $1)
             AND ($2::text IS NULL OR p.payment_method = $2)
             AND ($3::text IS NULL OR c.name ILIKE '%' || $3 || '%' OR s.business_name ILIKE '%' || $3 || '%'
                  OR p.transaction_reference ILIKE '%' || $3 || '%'
                  OR si.invoice_number ILIKE '%' || $3 || '%' OR po.po_number ILIKE '%' || $3 || '%')
             AND ($4::date IS NULL OR p.payment_date >= $4)
             AND ($5::date IS NULL OR p.payment_date <= $5)
           ORDER BY p.payment_date DESC, p.created_at DESC
           LIMIT $6 OFFSET $7"#,
        direction, method, search, q.from, q.to, q.per_page(), q.offset()
    )
    .fetch_all(&state.pool)
    .await?;

    let total: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM payments p
           LEFT JOIN customers c ON c.id = p.customer_id
           LEFT JOIN suppliers s ON s.id = p.supplier_id
           LEFT JOIN sales_invoices si ON p.reference_type = 'invoice' AND si.id = p.reference_id
           LEFT JOIN purchase_orders po ON p.reference_type = 'purchase' AND po.id = p.reference_id
           WHERE ($1::text IS NULL OR p.direction = $1)
             AND ($2::text IS NULL OR p.payment_method = $2)
             AND ($3::text IS NULL OR c.name ILIKE '%' || $3 || '%' OR s.business_name ILIKE '%' || $3 || '%'
                  OR p.transaction_reference ILIKE '%' || $3 || '%'
                  OR si.invoice_number ILIKE '%' || $3 || '%' OR po.po_number ILIKE '%' || $3 || '%')
             AND ($4::date IS NULL OR p.payment_date >= $4)
             AND ($5::date IS NULL OR p.payment_date <= $5)"#,
        direction, method, search, q.from, q.to
    )
    .fetch_one(&state.pool)
    .await?;

    let data = rows
        .into_iter()
        .map(|r| {
            json!({
                "id": r.id, "payment_date": r.payment_date, "date": r.payment_date,
                "amount": r.amount, "payment_method": r.payment_method, "method": r.payment_method,
                "direction": r.direction, "status": r.status,
                "reference_type": r.reference_type, "reference_id": r.reference_id,
                "document_number": r.document_number,
                "transaction_reference": r.transaction_reference, "reference": r.transaction_reference,
                "gateway_reference": r.gateway_reference, "notes": r.notes,
                "customer_name": r.customer_name, "supplier_name": r.supplier_name,
                "party_name": r.customer_name.or(r.supplier_name),
                "created_at": r.created_at,
            })
        })
        .collect();
    Ok(Json(paginated(data, total, &q)))
}

pub async fn get_payment(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "payments.record")?;
    let row = sqlx::query!("SELECT * FROM payments WHERE id = $1", id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Payment not found".into()))?;
    Ok(Json(json!({
        "id": row.id, "amount": row.amount, "payment_date": row.payment_date,
        "payment_method": row.payment_method, "direction": row.direction, "status": row.status,
        "reference_type": row.reference_type, "reference_id": row.reference_id,
        "transaction_reference": row.transaction_reference, "notes": row.notes
    })))
}

pub async fn create_razorpay_order(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(body): Json<Value>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "payments.record")?;

    let invoice_id = uuid(body.get("invoice_id")).ok_or_else(|| AppError::Validation("invoice_id is required".into()))?;
    let cfg = &state.config;
    if cfg.razorpay_key_id.is_empty() || cfg.razorpay_key_secret.is_empty() {
        return Err(AppError::ExternalService(
            "Razorpay is not configured. Add RAZORPAY_KEY_ID and RAZORPAY_KEY_SECRET to the environment.".into(),
        ));
    }

    let invoice = sqlx::query!("SELECT invoice_number, outstanding_amount, status FROM sales_invoices WHERE id = $1", invoice_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Invoice not found".into()))?;
    if invoice.status == "cancelled" || invoice.outstanding_amount <= Decimal::ZERO {
        return Err(AppError::BusinessLogic("Nothing is due on this invoice".into()));
    }

    let amount_paise = (invoice.outstanding_amount * Decimal::from(100))
        .round()
        .to_i64()
        .ok_or_else(|| AppError::Validation("Invalid amount".into()))?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let rp_resp = client
        .post("https://api.razorpay.com/v1/orders")
        .basic_auth(&cfg.razorpay_key_id, Some(&cfg.razorpay_key_secret))
        .json(&json!({
            "amount": amount_paise,
            "currency": "INR",
            "receipt": invoice.invoice_number,
            "notes": { "invoice_id": invoice_id.to_string() }
        }))
        .send()
        .await
        .map_err(|e| AppError::ExternalService(format!("Could not reach Razorpay: {}", e)))?;

    if !rp_resp.status().is_success() {
        let err: Value = rp_resp.json().await.unwrap_or_default();
        let msg = err.pointer("/error/description").and_then(|v| v.as_str()).unwrap_or("request rejected");
        return Err(AppError::ExternalService(format!("Razorpay error: {}", msg)));
    }

    let rp_order: Value = rp_resp.json().await.map_err(|e| AppError::ExternalService(e.to_string()))?;
    let order_id = rp_order
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::ExternalService("Razorpay did not return an order id".into()))?;

    sqlx::query!(
        r#"INSERT INTO payment_gateway_orders (invoice_id, gateway, gateway_order_id, amount, currency, status)
           VALUES ($1, 'razorpay', $2, $3, 'INR', 'created')"#,
        invoice_id, order_id, invoice.outstanding_amount
    )
    .execute(&state.pool)
    .await?;

    Ok(Json(json!({
        "razorpay_order_id": order_id,
        "amount": amount_paise,
        "currency": "INR",
        "key_id": cfg.razorpay_key_id
    })))
}

pub async fn razorpay_webhook(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> AppResult<Json<Value>> {
    let secret = &state.config.razorpay_webhook_secret;
    // Unsigned webhooks could mark any invoice as paid, so they are refused outright.
    if secret.is_empty() {
        return Err(AppError::ExternalService("Razorpay webhook secret is not configured".into()));
    }

    let signature = headers
        .get("x-razorpay-signature")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| hex::decode(s.trim()).ok())
        .ok_or_else(|| AppError::Unauthorized("Missing or malformed webhook signature".into()))?;

    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .map_err(|_| AppError::Internal("HMAC init failed".into()))?;
    mac.update(&body);
    mac.verify_slice(&signature)
        .map_err(|_| AppError::Unauthorized("Invalid webhook signature".into()))?;

    let payload: Value = serde_json::from_slice(&body).map_err(|_| AppError::Validation("Invalid JSON payload".into()))?;
    let event = payload.get("event").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let event_id = headers.get("x-razorpay-event-id").and_then(|v| v.to_str().ok()).map(String::from);

    // Record the raw event; a duplicate event id means Razorpay is retrying one we already handled.
    let inserted = sqlx::query!(
        r#"INSERT INTO payment_gateway_events (gateway, event_type, event_id, payload)
           VALUES ('razorpay', $1, $2, $3) ON CONFLICT (event_id) DO NOTHING"#,
        event, event_id, payload
    )
    .execute(&state.pool)
    .await?;
    if event_id.is_some() && inserted.rows_affected() == 0 {
        return Ok(Json(json!({"status": "duplicate"})));
    }

    if event == "payment.captured" {
        if let Some(entity) = payload.pointer("/payload/payment/entity") {
            apply_captured_payment(&state, entity).await?;
        }
    } else {
        tracing::info!("Ignoring Razorpay event: {}", event);
    }

    if let Some(eid) = &event_id {
        sqlx::query!("UPDATE payment_gateway_events SET processed = true WHERE event_id = $1", eid)
            .execute(&state.pool)
            .await
            .ok();
    }

    Ok(Json(json!({"status": "ok"})))
}

async fn apply_captured_payment(state: &AppState, entity: &Value) -> AppResult<()> {
    let order_id = entity.get("order_id").and_then(|v| v.as_str()).unwrap_or_default();
    let payment_id = entity.get("id").and_then(|v| v.as_str()).unwrap_or_default();
    let amount_paise = entity.get("amount").and_then(|v| v.as_i64()).unwrap_or(0);
    if order_id.is_empty() || payment_id.is_empty() || amount_paise <= 0 {
        return Ok(());
    }
    let amount = Decimal::new(amount_paise, 2);

    let mut tx = state.pool.begin().await?;
    let Some(order) = sqlx::query!(
        "SELECT invoice_id FROM payment_gateway_orders WHERE gateway_order_id = $1 AND gateway = 'razorpay' FOR UPDATE",
        order_id
    )
    .fetch_optional(&mut *tx)
    .await?
    else {
        tracing::warn!(order_id, "Razorpay payment for unknown order");
        return Ok(());
    };
    let Some(invoice_id) = order.invoice_id else { return Ok(()) };

    let already: bool = sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM payments WHERE gateway_reference = $1) AS "e!""#,
        payment_id
    )
    .fetch_one(&mut *tx)
    .await?;
    if already {
        return Ok(());
    }

    let Some(inv) = sqlx::query!(
        "SELECT customer_id, outstanding_amount FROM sales_invoices WHERE id = $1 FOR UPDATE",
        invoice_id
    )
    .fetch_optional(&mut *tx)
    .await?
    else {
        return Ok(());
    };

    let applied = amount.min(inv.outstanding_amount.max(Decimal::ZERO));
    let new_outstanding = inv.outstanding_amount - applied;
    let status = if new_outstanding <= Decimal::ZERO { "paid" } else { "partially_paid" };

    sqlx::query!(
        "UPDATE sales_invoices SET amount_paid = amount_paid + $1, outstanding_amount = $2, status = $3, updated_at = NOW() WHERE id = $4",
        applied, new_outstanding, status, invoice_id
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        r#"INSERT INTO payments (payment_date, amount, payment_method, direction, reference_type, reference_id, customer_id,
               gateway_reference, status, notes)
           VALUES ($1,$2,'razorpay','inbound','invoice',$3,$4,$5,'verified','Razorpay online payment')"#,
        today(), amount, invoice_id, inv.customer_id, payment_id
    )
    .execute(&mut *tx)
    .await?;

    if let Some(cid) = inv.customer_id {
        sqlx::query!(
            "UPDATE customers SET outstanding_balance = GREATEST(outstanding_balance - $1, 0) WHERE id = $2",
            applied, cid
        )
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query!(
        r#"UPDATE payment_reminders SET
               status = CASE WHEN $2::numeric <= 0 THEN 'settled' ELSE status END,
               outstanding_amount = GREATEST($2::numeric, 0)
           WHERE reference_type = 'invoice' AND reference_id = $1 AND status NOT IN ('settled','cancelled')"#,
        invoice_id, new_outstanding
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        "UPDATE payment_gateway_orders SET status = 'captured', gateway_payment_id = $1 WHERE gateway_order_id = $2",
        payment_id, order_id
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn upi_payment_details(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "invoices.create")?;
    let invoice = sqlx::query!("SELECT invoice_number, outstanding_amount FROM sales_invoices WHERE id = $1", id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Invoice not found".into()))?;

    // Company settings take precedence over environment defaults.
    let company = load_company(&state.pool).await?;
    let upi_id = company.upi_id.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| state.config.upi_merchant_id.clone());
    let merchant_name = company.upi_name.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| state.config.upi_merchant_name.clone());

    if upi_id.is_empty() {
        return Err(AppError::BusinessLogic("Set a UPI ID in Settings to accept UPI payments".into()));
    }

    let amount = invoice.outstanding_amount.round_dp(2);
    let note = format!("Invoice {}", invoice.invoice_number);
    let tr = invoice.invoice_number.clone();

    let upi_string = format!(
        "upi://pay?pa={}&pn={}&am={:.2}&tn={}&tr={}&cu=INR",
        urlencoding::encode(&upi_id),
        urlencoding::encode(&merchant_name),
        amount,
        urlencoding::encode(&note),
        urlencoding::encode(&tr)
    );

    use base64::{engine::general_purpose, Engine as _};
    use image::Luma;
    use qrcode::QrCode;

    let qr = QrCode::new(upi_string.as_bytes()).map_err(|e| AppError::Internal(format!("QR generation failed: {}", e)))?;
    let img = qr.render::<Luma<u8>>().min_dimensions(240, 240).build();
    let mut png_bytes: Vec<u8> = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png)
        .map_err(|e| AppError::Internal(format!("PNG encoding failed: {}", e)))?;

    Ok(Json(json!({
        "upi_id": upi_id,
        "merchant_name": merchant_name,
        "amount": amount,
        "reference": tr,
        "note": note,
        "upi_string": upi_string,
        "qr_code_base64": format!("data:image/png;base64,{}", general_purpose::STANDARD.encode(&png_bytes))
    })))
}
