use axum::{
    extract::{Request, State},
    http::header::AUTHORIZATION,
    middleware::Next,
    response::Response,
};

use crate::{
    auth::service::{load_authenticated_user, verify_access_token},
    errors::AppError,
    routes::AppState,
};

/// Applied only to the protected router, so public routes never reach it.
pub async fn require_auth(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    // CORS preflights carry no credentials.
    if request.method() == axum::http::Method::OPTIONS {
        return Ok(next.run(request).await);
    }

    let token = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .ok_or_else(|| AppError::Unauthorized("Please sign in to continue".to_string()))?;

    let claims = verify_access_token(token)?;
    let user_id = claims
        .sub
        .parse::<uuid::Uuid>()
        .map_err(|_| AppError::Unauthorized("Invalid session token".to_string()))?;

    // Re-read the user on every request so deactivation and permission
    // changes take effect immediately.
    let user = load_authenticated_user(&state.pool, user_id)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Account not found or deactivated".to_string()))?;

    request.extensions_mut().insert(user);
    Ok(next.run(request).await)
}
