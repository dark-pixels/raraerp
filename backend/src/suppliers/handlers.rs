use axum::{extract::{State, Extension, Path, Query}, Json};
use uuid::Uuid;
use crate::{auth::models::AuthenticatedUser, errors::{AppError, AppResult}, middleware::permission::require_permission, routes::AppState};

pub async fn list_suppliers(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(q): Query<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "purchases.read")?;
    let rows = sqlx::query!(
        r#"SELECT id, business_name, contact_person, phone, email, gstin,
                  outstanding_balance, is_active, created_at
           FROM suppliers ORDER BY business_name LIMIT 200"#
    ).fetch_all(&state.pool).await?;

    let data: Vec<_> = rows.iter().map(|r| serde_json::json!({
        "id": r.id, "business_name": r.business_name, "contact_person": r.contact_person,
        "phone": r.phone, "email": r.email, "gstin": r.gstin,
        "outstanding_balance": r.outstanding_balance, "is_active": r.is_active
    })).collect();
    Ok(Json(serde_json::json!({"data": data})))
}

pub async fn get_supplier(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "purchases.read")?;
    let row = sqlx::query!("SELECT * FROM suppliers WHERE id = $1", id)
        .fetch_optional(&state.pool).await?
        .ok_or_else(|| AppError::NotFound("Supplier not found".into()))?;
    Ok(Json(serde_json::json!({
        "id": row.id, "business_name": row.business_name, "contact_person": row.contact_person,
        "phone": row.phone, "email": row.email, "address": row.address, "gstin": row.gstin,
        "payment_terms_days": row.payment_terms_days, "credit_limit": row.credit_limit,
        "outstanding_balance": row.outstanding_balance, "is_active": row.is_active
    })))
}

pub async fn create_supplier(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "purchases.create")?;
    let name = body.get("business_name").and_then(|v| v.as_str()).ok_or_else(|| AppError::Validation("business_name required".into()))?;
    let phone = body.get("phone").and_then(|v| v.as_str()).unwrap_or("");
    let id = sqlx::query_scalar!(
        r#"INSERT INTO suppliers (business_name, contact_person, phone, email, address, gstin, payment_terms_days)
           VALUES ($1,$2,$3,$4,$5,$6,$7) RETURNING id"#,
        name,
        body.get("contact_person").and_then(|v| v.as_str()),
        phone,
        body.get("email").and_then(|v| v.as_str()),
        body.get("address").and_then(|v| v.as_str()),
        body.get("gstin").and_then(|v| v.as_str()),
        body.get("payment_terms_days").and_then(|v| v.as_i64()).map(|v| v as i32).unwrap_or(30)
    ).fetch_one(&state.pool).await?;
    Ok(Json(serde_json::json!({"id": id})))
}

pub async fn update_supplier(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "purchases.create")?;
    sqlx::query!(
        r#"UPDATE suppliers SET
            business_name = COALESCE($1, business_name),
            contact_person = COALESCE($2, contact_person),
            phone = COALESCE($3, phone),
            email = COALESCE($4, email),
            address = COALESCE($5, address)
           WHERE id = $6"#,
        body.get("business_name").and_then(|v| v.as_str()),
        body.get("contact_person").and_then(|v| v.as_str()),
        body.get("phone").and_then(|v| v.as_str()),
        body.get("email").and_then(|v| v.as_str()),
        body.get("address").and_then(|v| v.as_str()),
        id
    ).execute(&state.pool).await?;
    Ok(Json(serde_json::json!({"message": "Supplier updated"})))
}

pub async fn supplier_ledger(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
    Query(q): Query<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "finance.read")?;
    let rows = sqlx::query!(
        r#"SELECT p.id, p.payment_date, p.amount, p.payment_method, p.direction, p.status, p.notes
           FROM payments p
           WHERE p.supplier_id = $1
           ORDER BY p.payment_date DESC LIMIT 100"#, id
    ).fetch_all(&state.pool).await?;

    let data: Vec<_> = rows.iter().map(|r| serde_json::json!({
        "id": r.id, "date": r.payment_date, "amount": r.amount,
        "method": r.payment_method, "direction": r.direction, "status": r.status, "notes": r.notes
    })).collect();
    Ok(Json(serde_json::json!({"data": data})))
}
