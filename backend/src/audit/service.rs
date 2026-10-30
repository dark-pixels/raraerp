use uuid::Uuid;

use crate::database::DbPool;

/// Records an audit entry. Failures are logged but never fail the business
/// operation that triggered them.
pub async fn log_action(
    pool: &DbPool,
    user_id: Uuid,
    action: &str,
    resource_type: &str,
    resource_id: Option<&str>,
    changes: Option<serde_json::Value>,
) {
    let result = sqlx::query!(
        r#"INSERT INTO audit_logs (user_id, action, resource_type, resource_id, changes)
           VALUES ($1, $2, $3, $4, $5)"#,
        user_id,
        action,
        resource_type,
        resource_id,
        changes
    )
    .execute(pool)
    .await;

    if let Err(e) = result {
        tracing::warn!(action, resource_type, "Failed to write audit log: {}", e);
    }
}
