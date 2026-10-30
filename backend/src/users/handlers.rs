use axum::{
    extract::{Extension, Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    audit::service::log_action,
    auth::{
        models::AuthenticatedUser,
        service::{hash_password, validate_password_strength},
    },
    common::{non_empty, paginated, ListQuery},
    errors::{AppError, AppResult},
    middleware::permission::require_permission,
    routes::AppState,
};

const ROLES: &[&str] = &["super_admin", "admin", "staff"];

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct UserRecord {
    pub id: Uuid,
    pub email: String,
    pub full_name: String,
    pub role: String,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_login: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub full_name: String,
    pub password: String,
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub full_name: Option<String>,
    pub email: Option<String>,
    pub role: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePermissionsRequest {
    pub permissions: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ResetPasswordRequest {
    pub password: String,
}

fn rank(role: &str) -> u8 {
    match role {
        "super_admin" => 3,
        "admin" => 2,
        _ => 1,
    }
}

fn validate_role(role: &str) -> AppResult<()> {
    if ROLES.contains(&role) {
        Ok(())
    } else {
        Err(AppError::Validation("Role must be super_admin, admin or staff".into()))
    }
}

/// Only super admins manage admins and super admins; others may only manage staff.
fn ensure_can_manage(actor: &AuthenticatedUser, target_role: &str) -> AppResult<()> {
    if actor.role == "super_admin" || rank(target_role) < rank(&actor.role) {
        Ok(())
    } else {
        Err(AppError::Forbidden(format!("managing {} accounts", target_role.replace('_', " "))))
    }
}

fn validate_email(email: &str) -> AppResult<String> {
    let email = email.trim().to_lowercase();
    let valid = email.len() <= 254
        && email.split_once('@').map(|(l, d)| !l.is_empty() && d.contains('.') && !d.starts_with('.') && !d.ends_with('.')).unwrap_or(false)
        && !email.contains(char::is_whitespace);
    if valid { Ok(email) } else { Err(AppError::Validation("Enter a valid email address".into())) }
}

async fn target_role(state: &AppState, id: Uuid) -> AppResult<String> {
    sqlx::query_scalar!("SELECT role FROM users WHERE id = $1", id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))
}

async fn active_super_admins(state: &AppState) -> AppResult<i64> {
    Ok(sqlx::query_scalar!(r#"SELECT COUNT(*) AS "c!" FROM users WHERE role = 'super_admin' AND is_active"#)
        .fetch_one(&state.pool)
        .await?)
}

pub async fn list_users(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "users.read")?;
    let role = non_empty(&q.role);
    let search = q.search();

    let users = sqlx::query_as::<_, UserRecord>(
        r#"SELECT id, email, full_name, role, is_active, created_at,
                  (SELECT MAX(created_at) FROM login_events WHERE user_id = users.id AND success = true) AS last_login
           FROM users
           WHERE ($1::text IS NULL OR role = $1)
             AND ($2::text IS NULL OR email ILIKE '%' || $2 || '%' OR full_name ILIKE '%' || $2 || '%')
           ORDER BY is_active DESC, full_name
           LIMIT $3 OFFSET $4"#,
    )
    .bind(&role)
    .bind(&search)
    .bind(q.per_page())
    .bind(q.offset())
    .fetch_all(&state.pool)
    .await?;

    let total: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM users WHERE ($1::text IS NULL OR role = $1)
             AND ($2::text IS NULL OR email ILIKE '%' || $2 || '%' OR full_name ILIKE '%' || $2 || '%')"#,
    )
    .bind(&role)
    .bind(&search)
    .fetch_one(&state.pool)
    .await?;

    let data = users.into_iter().map(|u| serde_json::to_value(u).unwrap_or_default()).collect();
    Ok(Json(paginated(data, total, &q)))
}

pub async fn get_user(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "users.read")?;
    let record = sqlx::query_as::<_, UserRecord>(
        r#"SELECT id, email, full_name, role, is_active, created_at,
                  (SELECT MAX(created_at) FROM login_events WHERE user_id = users.id AND success = true) AS last_login
           FROM users WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    Ok(Json(json!(record)))
}

pub async fn create_user(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthenticatedUser>,
    Json(req): Json<CreateUserRequest>,
) -> AppResult<Json<Value>> {
    require_permission(&actor, "users.create")?;
    validate_role(&req.role)?;
    ensure_can_manage(&actor, &req.role)?;
    let email = validate_email(&req.email)?;
    let full_name = req.full_name.trim();
    if full_name.is_empty() {
        return Err(AppError::Validation("Full name is required".into()));
    }
    validate_password_strength(&req.password)?;

    let password_hash = hash_password(&req.password)?;
    let id = sqlx::query_scalar!(
        "INSERT INTO users (email, full_name, password_hash, role) VALUES ($1, $2, $3, $4) RETURNING id",
        email, full_name, password_hash, req.role
    )
    .fetch_one(&state.pool)
    .await?;

    log_action(&state.pool, actor.id, "user_created", "user", Some(&id.to_string()),
        Some(json!({"email": email, "role": req.role}))).await;
    Ok(Json(json!({ "id": id, "message": "User created successfully" })))
}

pub async fn update_user(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateUserRequest>,
) -> AppResult<Json<Value>> {
    require_permission(&actor, "users.update")?;
    let current_role = target_role(&state, id).await?;

    if id != actor.id {
        ensure_can_manage(&actor, &current_role)?;
    }
    if let Some(new_role) = &req.role {
        validate_role(new_role)?;
        if new_role != &current_role {
            if id == actor.id {
                return Err(AppError::BusinessLogic("You cannot change your own role".into()));
            }
            ensure_can_manage(&actor, new_role)?;
            if current_role == "super_admin" && active_super_admins(&state).await? <= 1 {
                return Err(AppError::BusinessLogic("Cannot demote the last active Super Admin".into()));
            }
        }
    }
    if req.is_active == Some(false) {
        if id == actor.id {
            return Err(AppError::BusinessLogic("You cannot deactivate your own account".into()));
        }
        if current_role == "super_admin" && active_super_admins(&state).await? <= 1 {
            return Err(AppError::BusinessLogic("Cannot deactivate the last active Super Admin".into()));
        }
    }
    let email = req.email.as_deref().filter(|e| !e.trim().is_empty()).map(validate_email).transpose()?;
    let full_name = req.full_name.as_deref().map(str::trim).filter(|n| !n.is_empty());

    sqlx::query!(
        r#"UPDATE users SET
               full_name = COALESCE($1, full_name),
               email = COALESCE($2, email),
               role = COALESCE($3, role),
               is_active = COALESCE($4, is_active),
               updated_at = NOW()
           WHERE id = $5"#,
        full_name, email, req.role, req.is_active, id
    )
    .execute(&state.pool)
    .await?;

    if req.is_active == Some(false) {
        sqlx::query!("UPDATE refresh_tokens SET revoked_at = NOW() WHERE user_id = $1 AND revoked_at IS NULL", id)
            .execute(&state.pool)
            .await?;
    }

    log_action(&state.pool, actor.id, "user_updated", "user", Some(&id.to_string()),
        Some(json!({"full_name": full_name, "email": email, "role": req.role, "is_active": req.is_active}))).await;
    Ok(Json(json!({ "message": "User updated" })))
}

pub async fn deactivate_user(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    require_permission(&actor, "users.deactivate")?;
    if id == actor.id {
        return Err(AppError::BusinessLogic("You cannot deactivate your own account".to_string()));
    }
    let role = target_role(&state, id).await?;
    ensure_can_manage(&actor, &role)?;
    if role == "super_admin" && active_super_admins(&state).await? <= 1 {
        return Err(AppError::BusinessLogic("Cannot deactivate the last active Super Admin".to_string()));
    }

    let mut tx = state.pool.begin().await?;
    sqlx::query!("UPDATE users SET is_active = false, updated_at = NOW() WHERE id = $1", id)
        .execute(&mut *tx)
        .await?;
    sqlx::query!("UPDATE refresh_tokens SET revoked_at = NOW() WHERE user_id = $1 AND revoked_at IS NULL", id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    log_action(&state.pool, actor.id, "user_deactivated", "user", Some(&id.to_string()), None).await;
    Ok(Json(json!({ "message": "User deactivated and signed out" })))
}

pub async fn reset_password(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<ResetPasswordRequest>,
) -> AppResult<Json<Value>> {
    require_permission(&actor, "users.update")?;
    let role = target_role(&state, id).await?;
    if id != actor.id {
        ensure_can_manage(&actor, &role)?;
    }
    validate_password_strength(&req.password)?;

    let hash = hash_password(&req.password)?;
    let mut tx = state.pool.begin().await?;
    sqlx::query!("UPDATE users SET password_hash = $1, updated_at = NOW() WHERE id = $2", hash, id)
        .execute(&mut *tx)
        .await?;
    sqlx::query!("UPDATE refresh_tokens SET revoked_at = NOW() WHERE user_id = $1 AND revoked_at IS NULL", id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    log_action(&state.pool, actor.id, "password_reset", "user", Some(&id.to_string()), None).await;
    Ok(Json(json!({ "message": "Password reset. The user has been signed out of all devices." })))
}

/// Returns the user's effective permissions and the role defaults they derive from.
pub async fn get_permissions(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    require_permission(&actor, "roles.manage")?;
    let role = target_role(&state, id).await?;

    let role_defaults: Vec<String> = sqlx::query_scalar!(
        "SELECT p.key FROM role_permissions rp JOIN permissions p ON p.id = rp.permission_id WHERE rp.role = $1 ORDER BY p.key",
        role
    )
    .fetch_all(&state.pool)
    .await?;

    let effective = crate::auth::service::load_authenticated_user(&state.pool, id)
        .await?
        .map(|u| u.permissions)
        .unwrap_or_default();

    Ok(Json(json!({ "role": role, "permissions": effective, "role_defaults": role_defaults })))
}

/// Sets the user's complete permission list. Differences from the role
/// defaults are stored as per-user grants or revocations.
pub async fn update_permissions(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdatePermissionsRequest>,
) -> AppResult<Json<Value>> {
    require_permission(&actor, "roles.manage")?;
    let role = target_role(&state, id).await?;
    ensure_can_manage(&actor, &role)?;
    if id == actor.id {
        return Err(AppError::BusinessLogic("You cannot change your own permissions".into()));
    }

    let mut tx = state.pool.begin().await?;
    sqlx::query!("DELETE FROM user_permission_overrides WHERE user_id = $1", id)
        .execute(&mut *tx)
        .await?;

    // Grants: requested keys the role does not already include.
    sqlx::query!(
        r#"INSERT INTO user_permission_overrides (user_id, permission_id, granted)
           SELECT $1, p.id, true FROM permissions p
           WHERE p.key = ANY($2)
             AND NOT EXISTS (SELECT 1 FROM role_permissions rp WHERE rp.role = $3 AND rp.permission_id = p.id)"#,
        id, &req.permissions, role
    )
    .execute(&mut *tx)
    .await?;

    // Revocations: role defaults that were left out of the request.
    sqlx::query!(
        r#"INSERT INTO user_permission_overrides (user_id, permission_id, granted)
           SELECT $1, rp.permission_id, false FROM role_permissions rp
           JOIN permissions p ON p.id = rp.permission_id
           WHERE rp.role = $3 AND NOT (p.key = ANY($2))"#,
        id, &req.permissions, role
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    log_action(&state.pool, actor.id, "permissions_updated", "user", Some(&id.to_string()),
        Some(json!({"permissions": req.permissions}))).await;
    Ok(Json(json!({ "message": "Permissions updated" })))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor(role: &str) -> AuthenticatedUser {
        AuthenticatedUser {
            id: Uuid::nil(),
            email: "a@b.c".into(),
            full_name: "A".into(),
            role: role.into(),
            is_active: true,
            permissions: vec![],
        }
    }

    #[test]
    fn role_hierarchy() {
        assert!(ensure_can_manage(&actor("super_admin"), "super_admin").is_ok());
        assert!(ensure_can_manage(&actor("admin"), "staff").is_ok());
        assert!(ensure_can_manage(&actor("admin"), "admin").is_err());
        assert!(ensure_can_manage(&actor("admin"), "super_admin").is_err());
        assert!(ensure_can_manage(&actor("staff"), "staff").is_err());
    }

    #[test]
    fn email_validation() {
        assert_eq!(validate_email(" Admin@Shop.COM ").unwrap(), "admin@shop.com");
        assert!(validate_email("no-at-sign").is_err());
        assert!(validate_email("a@nodot").is_err());
        assert!(validate_email("a b@c.com").is_err());
    }
}
