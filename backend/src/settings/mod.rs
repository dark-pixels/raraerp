use axum::{extract::{Extension, State}, Json};
use serde::Serialize;
use serde_json::Value;

use crate::{
    audit::service::log_action,
    auth::models::AuthenticatedUser,
    common::{normalize_gstin, text},
    database::DbPool,
    errors::{AppError, AppResult},
    middleware::permission::require_permission,
    routes::AppState,
};

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct CompanySettings {
    pub name: String,
    pub gstin: Option<String>,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub logo_url: Option<String>,
    pub invoice_prefix: String,
    pub upi_id: Option<String>,
    pub upi_name: Option<String>,
}

impl CompanySettings {
    /// Two-digit GST state code derived from the company GSTIN.
    pub fn state_code(&self) -> Option<&str> {
        self.gstin.as_deref().filter(|g| g.len() >= 2).map(|g| &g[..2])
    }
}

pub async fn load_company(pool: &DbPool) -> AppResult<CompanySettings> {
    let row = sqlx::query_as::<_, CompanySettings>(
        r#"SELECT name, gstin, address, phone, email, logo_url, invoice_prefix, upi_id, upi_name
           FROM company_settings ORDER BY created_at LIMIT 1"#,
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.unwrap_or(CompanySettings {
        name: "RARA Enterprise".into(),
        gstin: None,
        address: None,
        phone: None,
        email: None,
        logo_url: None,
        invoice_prefix: "INV".into(),
        upi_id: None,
        upi_name: None,
    }))
}

pub async fn get_company(
    State(state): State<AppState>,
    Extension(_): Extension<AuthenticatedUser>,
) -> AppResult<Json<CompanySettings>> {
    Ok(Json(load_company(&state.pool).await?))
}

pub async fn update_company(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(body): Json<Value>,
) -> AppResult<Json<CompanySettings>> {
    require_permission(&user, "settings.manage")?;

    let name = text(body.get("name")).ok_or_else(|| AppError::Validation("Company name is required".into()))?;
    let gstin = normalize_gstin(body.get("gstin"))?;
    let prefix = text(body.get("invoice_prefix")).unwrap_or_else(|| "INV".into()).to_uppercase();
    if prefix.len() > 10 || !prefix.chars().all(|c| c.is_ascii_alphanumeric() || c == '/' || c == '-') {
        return Err(AppError::Validation("Invoice prefix may contain up to 10 letters, digits, '-' or '/'".into()));
    }

    let mut tx = state.pool.begin().await?;
    let updated = sqlx::query!(
        r#"UPDATE company_settings SET name = $1, gstin = $2, address = $3, phone = $4, email = $5,
                  logo_url = $6, invoice_prefix = $7, upi_id = $8, upi_name = $9, updated_at = NOW()
           WHERE id = (SELECT id FROM company_settings ORDER BY created_at LIMIT 1)"#,
        name, gstin,
        text(body.get("address")), text(body.get("phone")), text(body.get("email")),
        text(body.get("logo_url")), prefix, text(body.get("upi_id")), text(body.get("upi_name"))
    )
    .execute(&mut *tx)
    .await?;

    if updated.rows_affected() == 0 {
        sqlx::query!(
            r#"INSERT INTO company_settings (name, gstin, address, phone, email, logo_url, invoice_prefix, upi_id, upi_name)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)"#,
            name, gstin,
            text(body.get("address")), text(body.get("phone")), text(body.get("email")),
            text(body.get("logo_url")), prefix, text(body.get("upi_id")), text(body.get("upi_name"))
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    log_action(&state.pool, user.id, "company_settings_updated", "company_settings", None, Some(body)).await;
    Ok(Json(load_company(&state.pool).await?))
}

/// All permission definitions, for the staff permission editor.
pub async fn list_permissions(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "users.read")?;
    let rows = sqlx::query!(
        r#"SELECT p.key, p.label, p.description,
                  COALESCE(array_agg(rp.role) FILTER (WHERE rp.role IS NOT NULL), '{}') AS "roles!"
           FROM permissions p LEFT JOIN role_permissions rp ON rp.permission_id = p.id
           GROUP BY p.id ORDER BY p.key"#
    )
    .fetch_all(&state.pool)
    .await?;
    let data: Vec<Value> = rows
        .into_iter()
        .map(|r| serde_json::json!({"key": r.key, "label": r.label, "description": r.description, "roles": r.roles}))
        .collect();
    Ok(Json(serde_json::json!({ "data": data })))
}
