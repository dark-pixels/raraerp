use crate::database::DbPool;
use crate::errors::AppResult;
use uuid::Uuid;
use chrono::Utc;

pub async fn log_action(
    pool: &DbPool,
    user_id: Uuid,
    action: &str,
    resource_type: &str,
    resource_id: Option<&str>,
    changes: Option<serde_json::Value>,
) -> AppResult<()> {
    sqlx::query!(
        r#"
        INSERT INTO audit_logs (user_id, action, resource_type, resource_id, changes)
        VALUES ($1, $2, $3, $4, $5)
        "#,
        user_id,
        action,
        resource_type,
        resource_id,
        changes
    )
    .execute(pool)
    .await?;
    Ok(())
}
