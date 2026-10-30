use axum::{
    extract::{Extension, State},
    http::HeaderMap,
    Json,
};
use chrono::Utc;

use crate::{
    audit::service::log_action,
    auth::{
        models::{
            AuthenticatedUser, ChangePasswordRequest, LoginRequest, LoginResponse, RefreshRequest,
            RefreshResponse, UserInfo,
        },
        service::{
            create_access_token, generate_refresh_token, hash_password, hash_refresh_token,
            load_authenticated_user, validate_password_strength, verify_password,
        },
    },
    common::client_ip,
    errors::{AppError, AppResult},
    routes::AppState,
};

/// Failed attempts allowed per account inside the lockout window.
const MAX_FAILED_LOGINS: i64 = 5;
const LOCKOUT_MINUTES: i64 = 15;

async fn issue_refresh_token(state: &AppState, user_id: uuid::Uuid) -> AppResult<String> {
    let token = generate_refresh_token();
    let expiry = Utc::now() + chrono::Duration::days(state.config.refresh_token_expiry_days);
    sqlx::query!(
        "INSERT INTO refresh_tokens (user_id, token_hash, expires_at) VALUES ($1, $2, $3)",
        user_id,
        hash_refresh_token(&token),
        expiry
    )
    .execute(&state.pool)
    .await?;
    Ok(token)
}

pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<LoginRequest>,
) -> AppResult<Json<LoginResponse>> {
    let email = req.email.trim().to_lowercase();
    if email.is_empty() || req.password.is_empty() {
        return Err(AppError::Validation("Email and password are required".into()));
    }
    let ip = client_ip(&headers);
    let user_agent = headers.get("user-agent").and_then(|v| v.to_str().ok()).map(String::from);

    let invalid = || AppError::Unauthorized("Invalid email or password".to_string());

    let user = sqlx::query!(
        "SELECT id, email, role, password_hash, is_active FROM users WHERE email = $1",
        email
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(invalid)?;

    let recent_failures: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM login_events
           WHERE user_id = $1 AND success = false
             AND created_at > NOW() - make_interval(mins => $2::int)
             AND created_at > COALESCE(
                 (SELECT MAX(created_at) FROM login_events WHERE user_id = $1 AND success = true),
                 '-infinity'::timestamptz)"#,
        user.id,
        LOCKOUT_MINUTES as i32
    )
    .fetch_one(&state.pool)
    .await?;

    if recent_failures >= MAX_FAILED_LOGINS {
        return Err(AppError::RateLimit(format!(
            "Too many failed sign-in attempts. Try again in {} minutes.",
            LOCKOUT_MINUTES
        )));
    }

    let password_ok = user
        .password_hash
        .as_deref()
        .map(|hash| verify_password(&req.password, hash))
        .unwrap_or(false);

    if !password_ok {
        sqlx::query!(
            "INSERT INTO login_events (user_id, ip_address, user_agent, success, failure_reason) VALUES ($1, $2, $3, false, 'invalid_password')",
            user.id, ip, user_agent
        )
        .execute(&state.pool)
        .await
        .ok();
        return Err(invalid());
    }

    if !user.is_active {
        return Err(AppError::Unauthorized("This account has been deactivated. Contact your administrator.".into()));
    }

    let access_token = create_access_token(&user.id.to_string(), &user.email, &user.role, state.config.jwt_expiry_seconds)?;
    let refresh_token = issue_refresh_token(&state, user.id).await?;

    sqlx::query!(
        "INSERT INTO login_events (user_id, ip_address, user_agent, success) VALUES ($1, $2, $3, true)",
        user.id, ip, user_agent
    )
    .execute(&state.pool)
    .await
    .ok();

    // Housekeeping: drop this user's expired or long-revoked refresh tokens.
    sqlx::query!(
        "DELETE FROM refresh_tokens WHERE user_id = $1 AND (expires_at < NOW() OR revoked_at < NOW() - INTERVAL '7 days')",
        user.id
    )
    .execute(&state.pool)
    .await
    .ok();

    let auth_user = load_authenticated_user(&state.pool, user.id)
        .await?
        .ok_or_else(invalid)?;

    Ok(Json(LoginResponse {
        access_token,
        refresh_token,
        expires_in: state.config.jwt_expiry_seconds,
        user: UserInfo::from(&auth_user),
    }))
}

pub async fn logout(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
) -> AppResult<Json<serde_json::Value>> {
    sqlx::query!(
        "UPDATE refresh_tokens SET revoked_at = NOW() WHERE user_id = $1 AND revoked_at IS NULL",
        user.id
    )
    .execute(&state.pool)
    .await?;

    Ok(Json(serde_json::json!({ "message": "Logged out successfully" })))
}

pub async fn refresh(
    State(state): State<AppState>,
    Json(req): Json<RefreshRequest>,
) -> AppResult<Json<RefreshResponse>> {
    let token_hash = hash_refresh_token(&req.refresh_token);

    // Revoke-and-read in one statement so a token can only be redeemed once,
    // even when two tabs refresh at the same moment.
    let record = sqlx::query!(
        r#"
        UPDATE refresh_tokens rt SET revoked_at = NOW()
        FROM users u
        WHERE u.id = rt.user_id
          AND rt.token_hash = $1
          AND rt.expires_at > NOW()
          AND rt.revoked_at IS NULL
        RETURNING rt.user_id, u.email, u.role, u.is_active
        "#,
        token_hash
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::Unauthorized("Your session has expired. Please sign in again.".to_string()))?;

    if !record.is_active {
        return Err(AppError::Unauthorized("This account has been deactivated".into()));
    }

    let access_token = create_access_token(&record.user_id.to_string(), &record.email, &record.role, state.config.jwt_expiry_seconds)?;
    let refresh_token = issue_refresh_token(&state, record.user_id).await?;

    Ok(Json(RefreshResponse {
        access_token,
        refresh_token,
        expires_in: state.config.jwt_expiry_seconds,
    }))
}

pub async fn me(Extension(user): Extension<AuthenticatedUser>) -> AppResult<Json<UserInfo>> {
    Ok(Json(UserInfo::from(&user)))
}

pub async fn change_password(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(req): Json<ChangePasswordRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let current_hash: Option<String> = sqlx::query_scalar!("SELECT password_hash FROM users WHERE id = $1", user.id)
        .fetch_one(&state.pool)
        .await?;

    if !current_hash.as_deref().map(|h| verify_password(&req.current_password, h)).unwrap_or(false) {
        return Err(AppError::Validation("Current password is incorrect".into()));
    }
    if req.current_password == req.new_password {
        return Err(AppError::Validation("New password must be different from the current password".into()));
    }
    validate_password_strength(&req.new_password)?;

    let new_hash = hash_password(&req.new_password)?;
    sqlx::query!("UPDATE users SET password_hash = $1, updated_at = NOW() WHERE id = $2", new_hash, user.id)
        .execute(&state.pool)
        .await?;

    // Sign out other devices.
    sqlx::query!(
        "UPDATE refresh_tokens SET revoked_at = NOW() WHERE user_id = $1 AND revoked_at IS NULL",
        user.id
    )
    .execute(&state.pool)
    .await?;

    // Keep this device signed in with a fresh refresh token.
    let refresh_token = issue_refresh_token(&state, user.id).await?;

    log_action(&state.pool, user.id, "password_changed", "user", Some(&user.id.to_string()), None).await;

    Ok(Json(serde_json::json!({
        "message": "Password changed. Other devices have been signed out.",
        "refresh_token": refresh_token
    })))
}
