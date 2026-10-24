use axum::{extract::{State, Extension, Path, Query}, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use rust_decimal::Decimal;
use sqlx::FromRow;
use crate::{
    auth::models::AuthenticatedUser,
    errors::{AppError, AppResult},
    middleware::permission::require_permission,
    routes::AppState,
    audit::service::log_action,
};

pub mod handlers;
pub mod service;
