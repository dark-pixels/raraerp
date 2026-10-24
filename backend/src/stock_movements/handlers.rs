use axum::{extract::{State, Extension, Path}, Json};
use uuid::Uuid;
use crate::{auth::models::AuthenticatedUser, errors::AppResult, middleware::permission::require_permission, routes::AppState};

pub async fn list_movements(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "inventory.read")?;
    let rows = sqlx::query!(
        r#"SELECT sm.id, sm.product_id, sm.quantity, sm.movement_type, sm.reference_document,
                  sm.timestamp, sm.reason,
                  p.name as product_name, p.sku
           FROM stock_movements sm JOIN products p ON p.id = sm.product_id
           ORDER BY sm.timestamp DESC LIMIT 200"#
    ).fetch_all(&state.pool).await?;

    let data: Vec<_> = rows.iter().map(|r| serde_json::json!({
        "id": r.id,
        "product_name": r.product_name,
        "sku": r.sku,
        "quantity": r.quantity,
        "type": r.movement_type,
        "reference": r.reference_document,
        "timestamp": r.timestamp,
        "reason": r.reason
    })).collect();

    Ok(Json(serde_json::json!({"data": data})))
}

pub async fn create_stock_count(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "inventory.count")?;

    let location_id: Uuid = body.get("location_id").and_then(|v| v.as_str()).and_then(|s| s.parse().ok())
        .ok_or_else(|| crate::errors::AppError::Validation("location_id required".into()))?;

    let id = sqlx::query_scalar!(
        "INSERT INTO stock_counts (location_id, created_by) VALUES ($1, $2) RETURNING id",
        location_id, user.id
    ).fetch_one(&state.pool).await?;

    Ok(Json(serde_json::json!({"id": id, "message": "Stock count started"})))
}
