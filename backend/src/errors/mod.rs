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

    #[error("Payment error: {0}")]
    Payment(String),

    #[error("Rate limit exceeded")]
    RateLimit,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_code, message) = match &self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "not_found", msg.as_str()),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, "unauthorized", msg.as_str()),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, "forbidden", msg.as_str()),
            AppError::Validation(msg) => (StatusCode::UNPROCESSABLE_ENTITY, "validation_error", msg.as_str()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, "conflict", msg.as_str()),
            AppError::BusinessLogic(msg) => (StatusCode::BAD_REQUEST, "business_logic_error", msg.as_str()),
            AppError::Database(e) => {
                eprintln!("[DATABASE ERROR] {}", e);
                tracing::error!("Database error: {}", e);
                let msg = format!("Database error: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": "database_error",
                        "message": msg
                    })),
                ).into_response();
            }
            AppError::Internal(msg) => {
                eprintln!("[INTERNAL ERROR] {}", msg);
                tracing::error!("Internal error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "internal_error", "An internal error occurred")
            }
            AppError::ExternalService(msg) => {
                tracing::warn!("External service error: {}", msg);
                (StatusCode::BAD_GATEWAY, "external_service_error", msg.as_str())
            }
            AppError::Payment(msg) => (StatusCode::BAD_REQUEST, "payment_error", msg.as_str()),
            AppError::RateLimit => (StatusCode::TOO_MANY_REQUESTS, "rate_limit", "Rate limit exceeded"),
        };

        (
            status,
            Json(json!({
                "error": error_code,
                "message": message
            })),
        )
            .into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;

// Convenient conversion helpers
impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}
