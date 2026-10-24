use axum::{extract::{State, Extension}, Json};
use crate::{auth::models::AuthenticatedUser, errors::AppResult, middleware::permission::require_permission, routes::AppState};

pub async fn list_audit_logs(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "settings.manage")?;
    let rows = sqlx::query!(
        r#"SELECT al.id, al.action, al.resource_type, al.resource_id, al.created_at,
                  u.full_name as user_name, u.email
           FROM audit_logs al JOIN users u ON u.id = al.user_id
           ORDER BY al.created_at DESC LIMIT 200"#
    ).fetch_all(&state.pool).await?;

    let data: Vec<_> = rows.iter().map(|r| serde_json::json!({
        "id": r.id, "action": r.action, "resource_type": r.resource_type,
        "resource_id": r.resource_id, "user_name": r.user_name,
        "email": r.email, "created_at": r.created_at
    })).collect();
    Ok(Json(serde_json::json!({"data": data})))
}
