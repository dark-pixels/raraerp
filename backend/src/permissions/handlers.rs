use axum::{extract::{State, Extension, Path, Query}, Json};
use crate::{auth::models::AuthenticatedUser, errors::AppResult, routes::AppState};

pub async fn list_permissions(State(_): State<AppState>, Extension(_): Extension<AuthenticatedUser>) -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({"data": [], "total": 0, "message": "Module: permissions"})))
}
