use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
    http::header::AUTHORIZATION,
};
use crate::{auth::service::verify_access_token, errors::AppError, routes::AppState};
use crate::auth::models::AuthenticatedUser;

pub async fn require_auth(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let auth_header = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized("Missing Authorization header".to_string()))?;

    if !auth_header.starts_with("Bearer ") {
        return Err(AppError::Unauthorized("Invalid Authorization format".to_string()));
    }

    let token = &auth_header["Bearer ".len()..];
    let claims = verify_access_token(token)
        .map_err(|e| AppError::Unauthorized(format!("Invalid token: {}", e)))?;

    // Parse UUID from claims subject
    let user_id = claims.sub.parse::<uuid::Uuid>()
        .map_err(|_| AppError::Unauthorized("Invalid user ID in token".to_string()))?;

    // Fetch user from DB to get current permissions and status
    let user = sqlx::query_as::<_, AuthenticatedUser>(
        r#"
        SELECT
            u.id,
            u.email,
            u.full_name,
            u.role,
            u.is_active,
            COALESCE(
                (SELECT json_agg(p.key)
                 FROM user_permission_overrides upo
                 JOIN permissions p ON p.id = upo.permission_id
                 WHERE upo.user_id = u.id AND upo.granted = true),
                (SELECT json_agg(p.key)
                 FROM role_permissions rp
                 JOIN permissions p ON p.id = rp.permission_id
                 WHERE rp.role = u.role),
                '[]'::json
            )::jsonb as permissions
        FROM users u
        WHERE u.id = $1 AND u.is_active = true
        "#
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::Unauthorized("User not found or inactive".to_string()))?;

    request.extensions_mut().insert(user);
    Ok(next.run(request).await)
}
