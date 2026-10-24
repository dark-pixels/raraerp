use axum::{extract::{State, Extension, Path, Query}, Json};
use uuid::Uuid;
use crate::{auth::models::AuthenticatedUser, errors::{AppError, AppResult}, middleware::permission::require_permission, routes::AppState};

pub async fn list_customers(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(q): Query<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "inventory.read")?;
    let rows = sqlx::query!(
        r#"SELECT id, name, business_name, phone, email, customer_type, gstin,
                  outstanding_balance, credit_limit, is_active
           FROM customers ORDER BY name LIMIT 200"#
    ).fetch_all(&state.pool).await?;

    let data: Vec<_> = rows.iter().map(|r| serde_json::json!({
        "id": r.id, "name": r.name, "business_name": r.business_name,
        "phone": r.phone, "email": r.email, "customer_type": r.customer_type,
        "gstin": r.gstin, "outstanding_balance": r.outstanding_balance,
        "credit_limit": r.credit_limit, "is_active": r.is_active
    })).collect();
    Ok(Json(serde_json::json!({"data": data})))
}

pub async fn get_customer(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "inventory.read")?;
    let row = sqlx::query!("SELECT * FROM customers WHERE id = $1", id)
        .fetch_optional(&state.pool).await?
        .ok_or_else(|| AppError::NotFound("Customer not found".into()))?;
    Ok(Json(serde_json::json!({
        "id": row.id, "name": row.name, "business_name": row.business_name,
        "phone": row.phone, "email": row.email, "billing_address": row.billing_address,
        "gstin": row.gstin, "customer_type": row.customer_type,
        "credit_limit": row.credit_limit, "credit_period_days": row.credit_period_days,
        "outstanding_balance": row.outstanding_balance, "is_active": row.is_active
    })))
}

pub async fn create_customer(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "sales.create")?;
    let name = body.get("name").and_then(|v| v.as_str()).ok_or_else(|| AppError::Validation("name required".into()))?;
    let phone = body.get("phone").and_then(|v| v.as_str()).unwrap_or("");
    let id = sqlx::query_scalar!(
        r#"INSERT INTO customers (name, business_name, phone, email, billing_address, gstin, customer_type, credit_limit, credit_period_days)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9) RETURNING id"#,
        name,
        body.get("business_name").and_then(|v| v.as_str()),
        phone,
        body.get("email").and_then(|v| v.as_str()),
        body.get("billing_address").and_then(|v| v.as_str()),
        body.get("gstin").and_then(|v| v.as_str()),
        body.get("customer_type").and_then(|v| v.as_str()).unwrap_or("retail"),
        body.get("credit_limit").and_then(|v| v.as_str()).and_then(|s| s.parse::<rust_decimal::Decimal>().ok()).unwrap_or(rust_decimal::Decimal::ZERO),
        body.get("credit_period_days").and_then(|v| v.as_i64()).map(|v| v as i32).unwrap_or(0)
    ).fetch_one(&state.pool).await?;
    Ok(Json(serde_json::json!({"id": id})))
}

pub async fn update_customer(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "sales.create")?;
    sqlx::query!(
        r#"UPDATE customers SET
            name = COALESCE($1, name),
            business_name = COALESCE($2, business_name),
            phone = COALESCE($3, phone),
            email = COALESCE($4, email)
           WHERE id = $5"#,
        body.get("name").and_then(|v| v.as_str()),
        body.get("business_name").and_then(|v| v.as_str()),
        body.get("phone").and_then(|v| v.as_str()),
        body.get("email").and_then(|v| v.as_str()),
        id
    ).execute(&state.pool).await?;
    Ok(Json(serde_json::json!({"message": "Customer updated"})))
}

pub async fn customer_ledger(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
    Query(_): Query<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "finance.read")?;
    let rows = sqlx::query!(
        r#"SELECT p.id, p.payment_date, p.amount, p.payment_method, p.direction, p.status
           FROM payments p WHERE p.customer_id = $1 ORDER BY p.payment_date DESC LIMIT 100"#, id
    ).fetch_all(&state.pool).await?;

    let invoices = sqlx::query!(
        r#"SELECT id, invoice_number, invoice_date, grand_total, outstanding_amount, status
           FROM sales_invoices WHERE customer_id = $1 ORDER BY invoice_date DESC LIMIT 50"#, id
    ).fetch_all(&state.pool).await?;

    let payment_data: Vec<_> = rows.iter().map(|r| serde_json::json!({"id": r.id, "date": r.payment_date, "amount": r.amount, "method": r.payment_method, "status": r.status})).collect();
    let invoice_data: Vec<_> = invoices.iter().map(|i| serde_json::json!({"id": i.id, "invoice_number": i.invoice_number, "date": i.invoice_date, "total": i.grand_total, "outstanding": i.outstanding_amount, "status": i.status})).collect();

    Ok(Json(serde_json::json!({"payments": payment_data, "invoices": invoice_data})))
}
