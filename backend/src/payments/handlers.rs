use axum::{extract::{State, Extension, Path, Query}, Json, response::Response, http::header};
use uuid::Uuid;
use rust_decimal::Decimal;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use crate::{auth::models::AuthenticatedUser, errors::{AppError, AppResult}, middleware::permission::require_permission, routes::AppState};

pub async fn list_payments(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(q): Query<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "payments.record")?;
    let rows = sqlx::query!(
        r#"SELECT p.id, p.payment_date, p.amount, p.payment_method, p.direction, p.status,
                  p.reference_type, p.transaction_reference, p.notes,
                  c.name as "customer_name?", s.business_name as "supplier_name?"
           FROM payments p
           LEFT JOIN customers c ON c.id = p.customer_id
           LEFT JOIN suppliers s ON s.id = p.supplier_id
           ORDER BY p.payment_date DESC LIMIT 100"#
    ).fetch_all(&state.pool).await?;

    let data: Vec<_> = rows.iter().map(|r| serde_json::json!({
        "id": r.id, "date": r.payment_date, "amount": r.amount,
        "method": r.payment_method, "direction": r.direction, "status": r.status,
        "reference_type": r.reference_type, "reference": r.transaction_reference,
        "customer_name": r.customer_name, "supplier_name": r.supplier_name
    })).collect();
    Ok(Json(serde_json::json!({"data": data})))
}

pub async fn get_payment(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "payments.record")?;
    let row = sqlx::query!("SELECT * FROM payments WHERE id = $1", id)
        .fetch_optional(&state.pool).await?
        .ok_or_else(|| AppError::NotFound("Payment not found".into()))?;
    Ok(Json(serde_json::json!({
        "id": row.id, "amount": row.amount, "date": row.payment_date,
        "method": row.payment_method, "status": row.status, "notes": row.notes
    })))
}

pub async fn create_razorpay_order(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "invoices.create")?;

    let invoice_id: Uuid = body.get("invoice_id").and_then(|v| v.as_str()).and_then(|s| s.parse().ok())
        .ok_or_else(|| AppError::Validation("invoice_id required".into()))?;

    let invoice = sqlx::query!("SELECT outstanding_amount FROM sales_invoices WHERE id = $1", invoice_id)
        .fetch_optional(&state.pool).await?
        .ok_or_else(|| AppError::NotFound("Invoice not found".into()))?;

    let amount_paise = (invoice.outstanding_amount * Decimal::from(100)).to_string().parse::<i64>()
        .map_err(|_| AppError::Validation("Invalid amount".into()))?;

    let key_id = std::env::var("RAZORPAY_KEY_ID").unwrap_or_default();
    let key_secret = std::env::var("RAZORPAY_KEY_SECRET").unwrap_or_default();

    if key_id.is_empty() || key_secret.is_empty() {
        return Err(AppError::ExternalService("Razorpay not configured. Please add RAZORPAY_KEY_ID and RAZORPAY_KEY_SECRET to environment.".into()));
    }

    let client = reqwest::Client::new();
    let rp_resp = client
        .post("https://api.razorpay.com/v1/orders")
        .basic_auth(&key_id, Some(&key_secret))
        .json(&serde_json::json!({
            "amount": amount_paise,
            "currency": "INR",
            "receipt": invoice_id.to_string(),
            "notes": { "invoice_id": invoice_id.to_string() }
        }))
        .send()
        .await
        .map_err(|e| AppError::ExternalService(format!("Razorpay request failed: {}", e)))?;

    if !rp_resp.status().is_success() {
        let err: serde_json::Value = rp_resp.json().await.unwrap_or_default();
        return Err(AppError::ExternalService(format!("Razorpay error: {}", err)));
    }

    let rp_order: serde_json::Value = rp_resp.json().await
        .map_err(|e| AppError::ExternalService(e.to_string()))?;

    // Store order reference
    sqlx::query!(
        r#"INSERT INTO payment_gateway_orders (invoice_id, gateway, gateway_order_id, amount, currency, status)
           VALUES ($1, 'razorpay', $2, $3, 'INR', 'created')"#,
        invoice_id,
        rp_order.get("id").and_then(|v| v.as_str()).unwrap_or(""),
        invoice.outstanding_amount
    ).execute(&state.pool).await.ok();

    Ok(Json(serde_json::json!({
        "razorpay_order_id": rp_order.get("id"),
        "amount": amount_paise,
        "currency": "INR",
        "key_id": key_id
    })))
}

pub async fn razorpay_webhook(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> AppResult<Json<serde_json::Value>> {
    let webhook_secret = std::env::var("RAZORPAY_WEBHOOK_SECRET").unwrap_or_default();

    if !webhook_secret.is_empty() {
        let signature = headers
            .get("x-razorpay-signature")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| AppError::Unauthorized("Missing webhook signature".into()))?;

        // Verify HMAC-SHA256 signature
        let mut mac = Hmac::<Sha256>::new_from_slice(webhook_secret.as_bytes())
            .map_err(|_| AppError::Internal("HMAC init failed".into()))?;
        mac.update(&body);
        let expected = hex::encode(mac.finalize().into_bytes());

        if expected != signature {
            return Err(AppError::Unauthorized("Invalid webhook signature".into()));
        }
    }

    let payload: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|_| AppError::Validation("Invalid JSON payload".into()))?;

    let event = payload.get("event").and_then(|v| v.as_str()).unwrap_or("");

    match event {
        "payment.captured" => {
            let payment_entity = payload.get("payload").and_then(|p| p.get("payment")).and_then(|p| p.get("entity"));
            if let Some(entity) = payment_entity {
                let order_id = entity.get("order_id").and_then(|v| v.as_str()).unwrap_or("");
                let payment_id = entity.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let amount_paise = entity.get("amount").and_then(|v| v.as_i64()).unwrap_or(0);
                let amount = Decimal::from(amount_paise) / Decimal::from(100);

                // Find invoice
                let order = sqlx::query!(
                    "SELECT invoice_id FROM payment_gateway_orders WHERE gateway_order_id = $1 AND gateway = 'razorpay'",
                    order_id
                ).fetch_optional(&state.pool).await?;

                if let Some(order) = order {
                    // Idempotency check
                    let already_exists: bool = sqlx::query_scalar!(
                        "SELECT EXISTS(SELECT 1 FROM payments WHERE gateway_reference = $1)", payment_id
                    ).fetch_one(&state.pool).await?.unwrap_or(false);

                    if !already_exists {
                        let invoice = sqlx::query!(
                            "SELECT customer_id, outstanding_amount FROM sales_invoices WHERE id = $1",
                            order.invoice_id
                        ).fetch_optional(&state.pool).await?;

                        if let Some(inv) = invoice {
                            let mut tx = state.pool.begin().await?;
                            let new_outstanding = inv.outstanding_amount - amount;
                            let status = if new_outstanding <= Decimal::ZERO { "paid" } else { "partially_paid" };

                            sqlx::query!(
                                "UPDATE sales_invoices SET amount_paid = amount_paid + $1, outstanding_amount = $2, status = $3 WHERE id = $4",
                                amount, new_outstanding.max(Decimal::ZERO), status, order.invoice_id
                            ).execute(&mut *tx).await?;

                            sqlx::query!(
                                r#"INSERT INTO payments (amount, payment_method, direction, reference_type, reference_id, customer_id, gateway_reference, status, recorded_by, notes)
                                   VALUES ($1,'razorpay','inbound','invoice',$2,$3,$4,'verified',gen_random_uuid(),'Razorpay webhook')"#,
                                amount, order.invoice_id, inv.customer_id, payment_id
                            ).execute(&mut *tx).await?;

                            sqlx::query!(
                                "UPDATE payment_gateway_orders SET status = 'captured', gateway_payment_id = $1 WHERE gateway_order_id = $2",
                                payment_id, order_id
                            ).execute(&mut *tx).await?;

                            tx.commit().await?;
                        }
                    }
                }
            }
        }
        _ => { tracing::info!("Unhandled Razorpay event: {}", event); }
    }

    Ok(Json(serde_json::json!({"status": "ok"})))
}

pub async fn upi_payment_details(
    State(state): State<AppState>,
    Extension(_): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let invoice = sqlx::query!(
        "SELECT invoice_number, outstanding_amount FROM sales_invoices WHERE id = $1", id
    ).fetch_optional(&state.pool).await?
     .ok_or_else(|| AppError::NotFound("Invoice not found".into()))?;

    let upi_id = std::env::var("UPI_MERCHANT_ID").unwrap_or_default();
    let merchant_name = std::env::var("UPI_MERCHANT_NAME").unwrap_or_else(|_| "RARA Enterprise".to_string());

    if upi_id.is_empty() {
        return Err(AppError::ExternalService("UPI merchant ID not configured".into()));
    }

    let amount = invoice.outstanding_amount;
    let note = format!("Invoice {}", invoice.invoice_number);
    let tr = format!("INV-{}", id.to_string().split('-').next().unwrap_or(""));

    // UPI Deep Link format
    let upi_string = format!(
        "upi://pay?pa={}&pn={}&am={}&tn={}&tr={}&cu=INR",
        urlencoding::encode(&upi_id),
        urlencoding::encode(&merchant_name),
        amount,
        urlencoding::encode(&note),
        urlencoding::encode(&tr)
    );

    // Generate QR code
    use qrcode::QrCode;
    use image::Luma;
    use base64::{Engine as _, engine::general_purpose};

    let qr = QrCode::new(upi_string.as_bytes())
        .map_err(|e| AppError::Internal(format!("QR generation failed: {}", e)))?;
    let image = qr.render::<Luma<u8>>().min_dimensions(200, 200).build();
    let mut png_bytes: Vec<u8> = Vec::new();
    image.write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png)
        .map_err(|e| AppError::Internal(format!("PNG encoding failed: {}", e)))?;

    let qr_base64 = general_purpose::STANDARD.encode(&png_bytes);

    Ok(Json(serde_json::json!({
        "upi_id": upi_id,
        "merchant_name": merchant_name,
        "amount": amount,
        "reference": tr,
        "note": note,
        "upi_string": upi_string,
        "qr_code_base64": format!("data:image/png;base64,{}", qr_base64)
    })))
}
