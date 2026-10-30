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
    common::{bool_field, dec_field, int, normalize_gstin, non_empty, paginated, required_text, text, ListQuery},
    errors::{AppError, AppResult},
    middleware::permission::require_permission,
    routes::AppState,
};

pub async fn list_customers(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "inventory.read")?;
    let search = q.search();
    let kind = non_empty(&q.kind);
    let status = q.status();

    let rows = sqlx::query!(
        r#"SELECT id, name, business_name, phone, email, billing_address, shipping_address, gstin, gst_verified,
                  customer_type, credit_limit, credit_period_days, opening_balance, outstanding_balance,
                  notes, is_active, created_at, COUNT(*) OVER () AS "full_count!"
           FROM customers
           WHERE ($1::text IS NULL OR name ILIKE '%' || $1 || '%' OR business_name ILIKE '%' || $1 || '%'
                  OR phone ILIKE '%' || $1 || '%' OR gstin ILIKE '%' || $1 || '%' OR email ILIKE '%' || $1 || '%')
             AND ($2::text IS NULL OR customer_type = $2)
             AND ($3::text IS NULL OR ($3 = 'active' AND is_active) OR ($3 = 'inactive' AND NOT is_active)
                  OR ($3 = 'outstanding' AND outstanding_balance > 0))
           ORDER BY is_active DESC, name
           LIMIT $4 OFFSET $5"#,
        search, kind, status, q.per_page(), q.offset()
    )
    .fetch_all(&state.pool)
    .await?;

    let total = rows.first().map(|r| r.full_count).unwrap_or(0);
    let data = rows
        .into_iter()
        .map(|r| {
            json!({
                "id": r.id, "name": r.name, "business_name": r.business_name,
                "phone": r.phone, "email": r.email, "billing_address": r.billing_address,
                "shipping_address": r.shipping_address, "gstin": r.gstin, "gst_verified": r.gst_verified,
                "customer_type": r.customer_type, "credit_limit": r.credit_limit,
                "credit_period_days": r.credit_period_days, "opening_balance": r.opening_balance,
                "outstanding_balance": r.outstanding_balance, "notes": r.notes,
                "is_active": r.is_active, "created_at": r.created_at,
            })
        })
        .collect();
    Ok(Json(paginated(data, total, &q)))
}

pub async fn get_customer(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "inventory.read")?;
    let row = sqlx::query!("SELECT * FROM customers WHERE id = $1", id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Customer not found".into()))?;
    Ok(Json(json!({
        "id": row.id, "name": row.name, "business_name": row.business_name,
        "phone": row.phone, "email": row.email, "billing_address": row.billing_address,
        "shipping_address": row.shipping_address, "gstin": row.gstin, "gst_verified": row.gst_verified,
        "customer_type": row.customer_type, "credit_limit": row.credit_limit,
        "credit_period_days": row.credit_period_days, "opening_balance": row.opening_balance,
        "outstanding_balance": row.outstanding_balance, "notes": row.notes, "is_active": row.is_active,
        "created_at": row.created_at
    })))
}

fn customer_type(body: &Value) -> AppResult<Option<String>> {
    match text(body.get("customer_type")) {
        None => Ok(None),
        Some(t) if t == "retail" || t == "wholesale" => Ok(Some(t)),
        Some(_) => Err(AppError::Validation("Customer type must be retail or wholesale".into())),
    }
}

fn non_negative(body: &Value, field: &str) -> AppResult<Option<Decimal>> {
    let v = dec_field(body, field)?;
    if v.map(|d| d < Decimal::ZERO).unwrap_or(false) {
        return Err(AppError::Validation(format!("{} cannot be negative", field.replace('_', " "))));
    }
    Ok(v)
}

pub async fn create_customer(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(body): Json<Value>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "sales.create")?;
    let name = required_text(&body, "name")?;
    let phone = text(body.get("phone")).unwrap_or_default();
    let gstin = normalize_gstin(body.get("gstin"))?;
    let opening = non_negative(&body, "opening_balance")?.unwrap_or(Decimal::ZERO);

    if !phone.is_empty() {
        let dup: bool = sqlx::query_scalar!(
            r#"SELECT EXISTS(SELECT 1 FROM customers WHERE phone = $1 AND is_active) AS "e!""#,
            phone
        )
        .fetch_one(&state.pool)
        .await?;
        if dup {
            return Err(AppError::Conflict(format!("A customer with phone {} already exists", phone)));
        }
    }

    let id = sqlx::query_scalar!(
        r#"INSERT INTO customers (name, business_name, phone, email, billing_address, shipping_address, gstin,
               customer_type, credit_limit, credit_period_days, opening_balance, outstanding_balance, notes)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$11,$12) RETURNING id"#,
        name,
        text(body.get("business_name")),
        phone,
        text(body.get("email")),
        text(body.get("billing_address")),
        text(body.get("shipping_address")),
        gstin,
        customer_type(&body)?.unwrap_or_else(|| "retail".into()),
        non_negative(&body, "credit_limit")?.unwrap_or(Decimal::ZERO),
        int(body.get("credit_period_days")).unwrap_or(0).max(0) as i32,
        opening,
        text(body.get("notes"))
    )
    .fetch_one(&state.pool)
    .await?;

    log_action(&state.pool, user.id, "customer_created", "customer", Some(&id.to_string()), Some(json!({"name": name}))).await;
    Ok(Json(json!({"id": id, "name": name, "message": "Customer created"})))
}

pub async fn update_customer(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
    Json(body): Json<Value>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "sales.create")?;
    let gstin = normalize_gstin(body.get("gstin"))?;
    let res = sqlx::query!(
        r#"UPDATE customers SET
            name = COALESCE($1, name),
            business_name = COALESCE($2, business_name),
            phone = COALESCE($3, phone),
            email = COALESCE($4, email),
            billing_address = COALESCE($5, billing_address),
            shipping_address = COALESCE($6, shipping_address),
            gstin = COALESCE($7, gstin),
            customer_type = COALESCE($8, customer_type),
            credit_limit = COALESCE($9, credit_limit),
            credit_period_days = COALESCE($10, credit_period_days),
            notes = COALESCE($11, notes),
            is_active = COALESCE($12, is_active)
           WHERE id = $13"#,
        text(body.get("name")),
        text(body.get("business_name")),
        text(body.get("phone")),
        text(body.get("email")),
        text(body.get("billing_address")),
        text(body.get("shipping_address")),
        gstin,
        customer_type(&body)?,
        non_negative(&body, "credit_limit")?,
        int(body.get("credit_period_days")).map(|d| d.max(0) as i32),
        text(body.get("notes")),
        bool_field(body.get("is_active")),
        id
    )
    .execute(&state.pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound("Customer not found".into()));
    }
    log_action(&state.pool, user.id, "customer_updated", "customer", Some(&id.to_string()), Some(body)).await;
    Ok(Json(json!({"message": "Customer updated"})))
}

/// Chronological statement of account with a running balance.
pub async fn customer_ledger(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "finance.read")?;
    let customer = sqlx::query!("SELECT name, opening_balance, outstanding_balance FROM customers WHERE id = $1", id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Customer not found".into()))?;

    let entries = sqlx::query!(
        r#"SELECT * FROM (
               SELECT si.invoice_date AS "date!", si.created_at AS "at!", 'invoice' AS "kind!",
                      si.invoice_number AS "reference!", si.grand_total AS "debit!", 0::numeric AS "credit!",
                      si.id AS "document_id!", si.status AS "status!"
               FROM sales_invoices si WHERE si.customer_id = $1 AND si.status <> 'cancelled'
               UNION ALL
               SELECT p.payment_date, p.created_at, 'payment', COALESCE(p.transaction_reference, p.payment_method),
                      0::numeric, p.amount, p.id, p.status
               FROM payments p WHERE p.customer_id = $1 AND p.direction = 'inbound' AND p.status = 'verified'
               UNION ALL
               SELECT cn.created_at::date, cn.created_at, 'credit_note', si.invoice_number,
                      0::numeric, cn.total, cn.id, cn.status
               FROM credit_notes cn JOIN sales_invoices si ON si.id = cn.original_invoice_id
               WHERE si.customer_id = $1 AND cn.status <> 'cancelled'
           ) e ORDER BY 1, 2"#,
        id
    )
    .fetch_all(&state.pool)
    .await?;

    let mut balance = customer.opening_balance;
    let rows: Vec<Value> = entries
        .into_iter()
        .map(|e| {
            balance += e.debit - e.credit;
            json!({
                "date": e.date, "type": e.kind, "reference": e.reference,
                "debit": e.debit, "credit": e.credit, "balance": balance,
                "document_id": e.document_id, "status": e.status
            })
        })
        .collect();

    Ok(Json(json!({
        "customer_name": customer.name,
        "opening_balance": customer.opening_balance,
        "closing_balance": balance,
        "outstanding_balance": customer.outstanding_balance,
        "entries": rows
    })))
}
