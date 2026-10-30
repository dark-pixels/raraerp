use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: insufficient permissions for {0}")]
    Forbidden(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Business logic error: {0}")]
    BusinessLogic(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("External service error: {0}")]
    ExternalService(String),

    #[error("{0}")]
    RateLimit(String),
}

/// Translates well-known PostgreSQL constraint violations into client errors
/// so users see an actionable message instead of a generic 500.
fn classify_database_error(e: &sqlx::Error) -> Option<(StatusCode, &'static str, String)> {
    match e {
        sqlx::Error::RowNotFound => Some((StatusCode::NOT_FOUND, "not_found", "Record not found".into())),
        sqlx::Error::Database(db) => {
            let constraint = db.constraint().unwrap_or_default();
            match db.code().as_deref() {
                Some("23505") => {
                    let what = if constraint.contains("sku") {
                        "A product with this SKU already exists"
                    } else if constraint.contains("barcode") {
                        "A product with this barcode already exists"
                    } else if constraint.contains("email") {
                        "This email is already registered"
                    } else if constraint.contains("code") {
                        "This code is already in use"
                    } else if constraint.contains("name") || constraint.contains("slug") {
                        "A record with this name already exists"
                    } else {
                        "A record with these details already exists"
                    };
                    Some((StatusCode::CONFLICT, "conflict", what.into()))
                }
                Some("23503") => Some((
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "validation_error",
                    "A referenced record does not exist, or this record is still in use".into(),
                )),
                Some("23514") => {
                    let msg = if constraint == "non_negative_stock" {
                        "Insufficient stock for this operation".to_string()
                    } else {
                        "One or more values are not allowed".to_string()
                    };
                    Some((StatusCode::UNPROCESSABLE_ENTITY, "validation_error", msg))
                }
                Some("23502") => Some((
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "validation_error",
                    "A required field is missing".into(),
                )),
                Some("22P02") | Some("22003") | Some("22007") | Some("22008") => Some((
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "validation_error",
                    "One or more values have an invalid format".into(),
                )),
                _ => None,
            }
        }
        _ => None,
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_code, message): (StatusCode, &str, String) = match &self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "not_found", msg.clone()),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, "unauthorized", msg.clone()),
            AppError::Forbidden(perm) => (
                StatusCode::FORBIDDEN,
                "forbidden",
                format!("You do not have permission to perform this action ({})", perm),
            ),
            AppError::Validation(msg) => (StatusCode::UNPROCESSABLE_ENTITY, "validation_error", msg.clone()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, "conflict", msg.clone()),
            AppError::BusinessLogic(msg) => (StatusCode::BAD_REQUEST, "business_logic_error", msg.clone()),
            AppError::Database(e) => match classify_database_error(e) {
                Some((status, code, msg)) => {
                    tracing::warn!("Database constraint error: {}", e);
                    (status, code, msg)
                }
                None => {
                    // Never leak SQL details to clients; they stay in the server log.
                    tracing::error!("Database error: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "database_error",
                        "A database error occurred. Please try again.".into(),
                    )
                }
            },
            AppError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "internal_error", "An internal error occurred".into())
            }
            AppError::ExternalService(msg) => {
                tracing::warn!("External service error: {}", msg);
                (StatusCode::BAD_GATEWAY, "external_service_error", msg.clone())
            }
            AppError::RateLimit(msg) => (StatusCode::TOO_MANY_REQUESTS, "rate_limit", msg.clone()),
        };

        (status, Json(json!({ "error": error_code, "message": message }))).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}
