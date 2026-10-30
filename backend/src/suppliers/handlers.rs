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
    common::{bool_field, dec_field, int, normalize_gstin, paginated, required_text, text, ListQuery},
    errors::{AppError, AppResult},
    middleware::permission::require_permission,
    routes::AppState,
};

pub async fn list_suppliers(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "purchases.read")?;
    let search = q.search();
    let status = q.status();

    let rows = sqlx::query!(
        r#"SELECT id, business_name, contact_person, phone, email, address, gstin, gst_verified,
                  payment_terms_days, credit_limit, opening_balance, outstanding_balance, notes,
                  is_active, created_at, COUNT(*) OVER () AS "full_count!"
           FROM suppliers
           WHERE ($1::text IS NULL OR business_name ILIKE '%' || $1 || '%' OR contact_person ILIKE '%' || $1 || '%'
                  OR phone ILIKE '%' || $1 || '%' OR gstin ILIKE '%' || $1 || '%')
             AND ($2::text IS NULL OR ($2 = 'active' AND is_active) OR ($2 = 'inactive' AND NOT is_active)
                  OR ($2 = 'outstanding' AND outstanding_balance > 0))
           ORDER BY is_active DESC, business_name
           LIMIT $3 OFFSET $4"#,
        search, status, q.per_page(), q.offset()
    )
    .fetch_all(&state.pool)
    .await?;

    let total = rows.first().map(|r| r.full_count).unwrap_or(0);
    let data = rows
        .into_iter()
        .map(|r| {
            json!({
                "id": r.id, "business_name": r.business_name, "contact_person": r.contact_person,
                "phone": r.phone, "email": r.email, "address": r.address, "gstin": r.gstin,
                "gst_verified": r.gst_verified, "payment_terms_days": r.payment_terms_days,
                "credit_limit": r.credit_limit, "opening_balance": r.opening_balance,
                "outstanding_balance": r.outstanding_balance, "notes": r.notes,
                "is_active": r.is_active, "created_at": r.created_at,
            })
        })
        .collect();
    Ok(Json(paginated(data, total, &q)))
}

pub async fn get_supplier(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "purchases.read")?;
    let row = sqlx::query!("SELECT * FROM suppliers WHERE id = $1", id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Supplier not found".into()))?;
    Ok(Json(json!({
        "id": row.id, "business_name": row.business_name, "contact_person": row.contact_person,
        "phone": row.phone, "email": row.email, "address": row.address, "gstin": row.gstin,
        "gst_verified": row.gst_verified, "payment_terms_days": row.payment_terms_days,
        "credit_limit": row.credit_limit, "opening_balance": row.opening_balance,
        "outstanding_balance": row.outstanding_balance, "notes": row.notes, "is_active": row.is_active,
        "created_at": row.created_at
    })))
}

fn non_negative(body: &Value, field: &str) -> AppResult<Option<Decimal>> {
    let v = dec_field(body, field)?;
    if v.map(|d| d < Decimal::ZERO).unwrap_or(false) {
        return Err(AppError::Validation(format!("{} cannot be negative", field.replace('_', " "))));
    }
    Ok(v)
}

pub async fn create_supplier(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(body): Json<Value>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "purchases.create")?;
    let name = required_text(&body, "business_name")?;
    let gstin = normalize_gstin(body.get("gstin"))?;
    let opening = non_negative(&body, "opening_balance")?.unwrap_or(Decimal::ZERO);

    if let Some(g) = &gstin {
        let dup: bool = sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM suppliers WHERE gstin = $1) AS "e!""#, g)
            .fetch_one(&state.pool)
            .await?;
        if dup {
            return Err(AppError::Conflict(format!("A supplier with GSTIN {} already exists", g)));
        }
    }

    let id = sqlx::query_scalar!(
        r#"INSERT INTO suppliers (business_name, contact_person, phone, email, address, gstin, payment_terms_days,
               credit_limit, opening_balance, outstanding_balance, notes)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$9,$10) RETURNING id"#,
        name,
        text(body.get("contact_person")),
        text(body.get("phone")).unwrap_or_default(),
        text(body.get("email")),
        text(body.get("address")),
        gstin,
        int(body.get("payment_terms_days")).unwrap_or(30).max(0) as i32,
        non_negative(&body, "credit_limit")?.unwrap_or(Decimal::ZERO),
        opening,
        text(body.get("notes"))
    )
    .fetch_one(&state.pool)
    .await?;

    log_action(&state.pool, user.id, "supplier_created", "supplier", Some(&id.to_string()), Some(json!({"name": name}))).await;
    Ok(Json(json!({"id": id, "business_name": name, "message": "Supplier created"})))
}

pub async fn update_supplier(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
    Json(body): Json<Value>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "purchases.create")?;
    let gstin = normalize_gstin(body.get("gstin"))?;
    let res = sqlx::query!(
        r#"UPDATE suppliers SET
            business_name = COALESCE($1, business_name),
            contact_person = COALESCE($2, contact_person),
            phone = COALESCE($3, phone),
            email = COALESCE($4, email),
            address = COALESCE($5, address),
            gstin = COALESCE($6, gstin),
            payment_terms_days = COALESCE($7, payment_terms_days),
            credit_limit = COALESCE($8, credit_limit),
            notes = COALESCE($9, notes),
            is_active = COALESCE($10, is_active)
           WHERE id = $11"#,
        text(body.get("business_name")),
        text(body.get("contact_person")),
        text(body.get("phone")),
        text(body.get("email")),
        text(body.get("address")),
        gstin,
        int(body.get("payment_terms_days")).map(|d| d.max(0) as i32),
        non_negative(&body, "credit_limit")?,
        text(body.get("notes")),
        bool_field(body.get("is_active")),
        id
    )
    .execute(&state.pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound("Supplier not found".into()));
    }
    log_action(&state.pool, user.id, "supplier_updated", "supplier", Some(&id.to_string()), Some(body)).await;
    Ok(Json(json!({"message": "Supplier updated"})))
}

/// Chronological statement of account (amounts owed to the supplier) with a running balance.
pub async fn supplier_ledger(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "finance.read")?;
    let supplier = sqlx::query!("SELECT business_name, opening_balance, outstanding_balance FROM suppliers WHERE id = $1", id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Supplier not found".into()))?;

    let entries = sqlx::query!(
        r#"SELECT * FROM (
               SELECT po.purchase_date AS "date!", po.created_at AS "at!", 'purchase' AS "kind!",
                      po.po_number AS "reference!", po.grand_total AS "credit!", 0::numeric AS "debit!",
                      po.id AS "document_id!", po.status AS "status!"
               FROM purchase_orders po WHERE po.supplier_id = $1 AND po.status <> 'cancelled'
               UNION ALL
               SELECT p.payment_date, p.created_at, 'payment', COALESCE(p.transaction_reference, p.payment_method),
                      0::numeric, p.amount, p.id, p.status
               FROM payments p WHERE p.supplier_id = $1 AND p.direction = 'outbound' AND p.status = 'verified'
           ) e ORDER BY 1, 2"#,
        id
    )
    .fetch_all(&state.pool)
    .await?;

    let mut balance = supplier.opening_balance;
    let rows: Vec<Value> = entries
        .into_iter()
        .map(|e| {
            balance += e.credit - e.debit;
            json!({
                "date": e.date, "type": e.kind, "reference": e.reference,
                "credit": e.credit, "debit": e.debit, "balance": balance,
                "document_id": e.document_id, "status": e.status
            })
        })
        .collect();

    Ok(Json(json!({
        "supplier_name": supplier.business_name,
        "opening_balance": supplier.opening_balance,
        "closing_balance": balance,
        "outstanding_balance": supplier.outstanding_balance,
        "entries": rows,
        "data": rows
    })))
}
