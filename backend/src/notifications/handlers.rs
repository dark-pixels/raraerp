use axum::{extract::{State, Extension, Path}, Json};
use uuid::Uuid;
use crate::{auth::models::AuthenticatedUser, errors::AppResult, routes::AppState};

pub async fn list_notifications(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
) -> AppResult<Json<serde_json::Value>> {
    let rows = sqlx::query!(
        r#"SELECT id, type, title, body, reference_type, reference_id, is_read, created_at
           FROM notifications WHERE user_id = $1 ORDER BY created_at DESC LIMIT 50"#,
        user.id
    ).fetch_all(&state.pool).await?;

    let data: Vec<_> = rows.iter().map(|r| serde_json::json!({
        "id": r.id, "type": r.r#type, "title": r.title, "body": r.body,
        "reference_type": r.reference_type, "reference_id": r.reference_id,
        "is_read": r.is_read, "created_at": r.created_at
    })).collect();

    let unread: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND is_read = false", user.id
    ).fetch_one(&state.pool).await?.unwrap_or(0);

    Ok(Json(serde_json::json!({"data": data, "unread_count": unread})))
}

pub async fn mark_read(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    sqlx::query!(
        "UPDATE notifications SET is_read = true WHERE id = $1 AND user_id = $2", id, user.id
    ).execute(&state.pool).await?;
    Ok(Json(serde_json::json!({"message": "Marked as read"})))
}

pub async fn mark_all_read(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
) -> AppResult<Json<serde_json::Value>> {
    sqlx::query!(
        "UPDATE notifications SET is_read = true WHERE user_id = $1 AND is_read = false", user.id
    ).execute(&state.pool).await?;
    Ok(Json(serde_json::json!({"message": "All notifications marked as read"})))
}
