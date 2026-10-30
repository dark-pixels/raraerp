use axum::{extract::{Extension, Query, State}, Json};

use crate::{
    auth::models::AuthenticatedUser,
    common::{paginated, ListQuery},
    errors::AppResult,
    middleware::permission::require_permission,
    routes::AppState,
};

pub async fn list_audit_logs(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "settings.manage")?;
    let search = q.search();

    let rows = sqlx::query!(
        r#"SELECT al.id, al.action, al.resource_type, al.resource_id, al.changes, al.ip_address, al.created_at,
                  u.full_name AS user_name, u.email
           FROM audit_logs al JOIN users u ON u.id = al.user_id
           WHERE ($1::text IS NULL OR al.action ILIKE '%' || $1 || '%'
                  OR al.resource_type ILIKE '%' || $1 || '%' OR u.email ILIKE '%' || $1 || '%')
             AND ($2::date IS NULL OR al.created_at >= $2::date)
             AND ($3::date IS NULL OR al.created_at < $3::date + 1)
           ORDER BY al.created_at DESC
           LIMIT $4 OFFSET $5"#,
        search, q.from, q.to, q.per_page(), q.offset()
    )
    .fetch_all(&state.pool)
    .await?;

    let total: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM audit_logs al JOIN users u ON u.id = al.user_id
           WHERE ($1::text IS NULL OR al.action ILIKE '%' || $1 || '%'
                  OR al.resource_type ILIKE '%' || $1 || '%' OR u.email ILIKE '%' || $1 || '%')
             AND ($2::date IS NULL OR al.created_at >= $2::date)
             AND ($3::date IS NULL OR al.created_at < $3::date + 1)"#,
        search, q.from, q.to
    )
    .fetch_one(&state.pool)
    .await?;

    let data = rows
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "action": r.action,
                "resource_type": r.resource_type,
                "entity_type": r.resource_type,
                "resource_id": r.resource_id,
                "entity_id": r.resource_id,
                "changes": r.changes,
                "description": r.changes.as_ref().map(|c| c.to_string()),
                "ip_address": r.ip_address,
                "user_name": r.user_name,
                "email": r.email,
                "user_email": r.email,
                "created_at": r.created_at,
            })
        })
        .collect();

    Ok(Json(paginated(data, total, &q)))
}
