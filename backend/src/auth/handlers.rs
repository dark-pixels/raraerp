use axum::{extract::{State, Extension}, Json};
use chrono::Utc;
use sha2::Digest;
use crate::{
    auth::{
        models::{AuthenticatedUser, LoginRequest, LoginResponse, RefreshRequest, RefreshResponse, UserInfo},
        service::{create_access_token, verify_password, generate_refresh_token},
    },
    errors::{AppError, AppResult},
    routes::AppState,
};

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> AppResult<Json<LoginResponse>> {
    let user = sqlx::query!(
        r#"
        SELECT id, email, full_name, role, password_hash, is_active
        FROM users WHERE email = $1
        "#,
        req.email.to_lowercase()
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::Unauthorized("Invalid email or password".to_string()))?;

    if !user.is_active {
        return Err(AppError::Unauthorized("Account is inactive".to_string()));
    }

    let hash = user.password_hash
        .ok_or_else(|| AppError::Unauthorized("Password not set".to_string()))?;

    if !verify_password(&req.password, &hash)? {
        // Log failed attempt
        sqlx::query!(
            "INSERT INTO login_events (user_id, ip_address, success, failure_reason) VALUES ($1, $2, false, $3)",
            user.id, "unknown", "Invalid password"
        )
        .execute(&state.pool)
        .await.ok();
        return Err(AppError::Unauthorized("Invalid email or password".to_string()));
    }

    // Create tokens
    let access_token = create_access_token(&user.id.to_string(), &user.email, &user.role)?;
    let refresh_token = generate_refresh_token();
    let refresh_expiry = Utc::now() + chrono::Duration::days(30);

    // Store refresh token
    sqlx::query!(
        r#"INSERT INTO refresh_tokens (user_id, token_hash, expires_at) VALUES ($1, $2, $3)"#,
        user.id,
        hex::encode(sha2::Sha256::digest(refresh_token.as_bytes())),
        refresh_expiry
    )
    .execute(&state.pool)
    .await?;

    // Log success
    sqlx::query!(
        "INSERT INTO login_events (user_id, ip_address, success) VALUES ($1, $2, true)",
        user.id, "unknown"
    )
    .execute(&state.pool)
    .await.ok();

    // Get user permissions
    let permissions: Vec<String> = sqlx::query_scalar!(
        r#"
        SELECT p.key FROM role_permissions rp
        JOIN permissions p ON p.id = rp.permission_id
        WHERE rp.role = $1
        "#,
        user.role
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    Ok(Json(LoginResponse {
        access_token,
        refresh_token,
        expires_in: 3600,
        user: UserInfo {
            id: user.id.to_string(),
            email: user.email,
            full_name: user.full_name,
            role: user.role,
            permissions,
            is_active: user.is_active,
        },
    }))
}

pub async fn logout(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
) -> AppResult<Json<serde_json::Value>> {
    // Revoke all refresh tokens for this user
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
    use sha2::Digest;
    let token_hash = hex::encode(sha2::Sha256::digest(req.refresh_token.as_bytes()));

    let record = sqlx::query!(
        r#"
        SELECT rt.user_id, u.email, u.role, u.is_active
        FROM refresh_tokens rt
        JOIN users u ON u.id = rt.user_id
        WHERE rt.token_hash = $1
          AND rt.expires_at > NOW()
          AND rt.revoked_at IS NULL
        "#,
        token_hash
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::Unauthorized("Invalid or expired refresh token".to_string()))?;

    if !record.is_active {
        return Err(AppError::Unauthorized("Account is inactive".to_string()));
    }

    // Revoke old token (rotation)
    sqlx::query!(
        "UPDATE refresh_tokens SET revoked_at = NOW() WHERE token_hash = $1",
        token_hash
    )
    .execute(&state.pool)
    .await?;

    let access_token = create_access_token(
        &record.user_id.to_string(),
        &record.email,
        &record.role,
    )?;

    Ok(Json(RefreshResponse {
        access_token,
        expires_in: 3600,
    }))
}

pub async fn me(
    Extension(user): Extension<AuthenticatedUser>,
) -> AppResult<Json<UserInfo>> {
    Ok(Json(UserInfo::from(&user)))
}
