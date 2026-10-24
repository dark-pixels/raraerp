use axum::{extract::{State, Extension, Path, Query}, Json, http::StatusCode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::{
    auth::{models::AuthenticatedUser, service::hash_password},
    errors::{AppError, AppResult},
    middleware::permission::require_permission,
    routes::AppState,
};

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
pub struct ListUsersQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub role: Option<String>,
    pub search: Option<String>,
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
    pub role: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePermissionsRequest {
    pub permissions: Vec<String>,
}

pub async fn list_users(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(q): Query<ListUsersQuery>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "users.read")?;

    let page = q.page.unwrap_or(1).max(1);
    let per_page = q.per_page.unwrap_or(20).min(100);
    let offset = (page - 1) * per_page;

    let users = sqlx::query_as::<_, UserRecord>(
        r#"
        SELECT id, email, full_name, role, is_active, created_at,
               (SELECT MAX(created_at) FROM login_events WHERE user_id = users.id AND success = true) as last_login
        FROM users
        WHERE ($1::text IS NULL OR role = $1)
          AND ($2::text IS NULL OR (email ILIKE '%' || $2 || '%' OR full_name ILIKE '%' || $2 || '%'))
        ORDER BY created_at DESC
        LIMIT $3 OFFSET $4
        "#
    )
    .bind(q.role)
    .bind(q.search)
    .bind(per_page)
    .bind(offset)
    .fetch_all(&state.pool)
    .await?;

    let total: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM users").fetch_one(&state.pool).await?.unwrap_or(0);

    Ok(Json(serde_json::json!({
        "data": users,
        "total": total,
        "page": page,
        "per_page": per_page,
        "total_pages": (total as f64 / per_page as f64).ceil() as i64
    })))
}

pub async fn get_user(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "users.read")?;

    let record = sqlx::query_as::<_, UserRecord>(
        r#"
        SELECT id, email, full_name, role, is_active, created_at,
               (SELECT MAX(created_at) FROM login_events WHERE user_id = users.id AND success = true) as last_login
        FROM users WHERE id = $1
        "#
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("User {} not found", id)))?;

    Ok(Json(serde_json::json!(record)))
}

pub async fn create_user(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthenticatedUser>,
    Json(req): Json<CreateUserRequest>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&auth_user, "users.create")?;

    // Only super_admin can create admins
    if req.role == "super_admin" && auth_user.role != "super_admin" {
        return Err(AppError::Forbidden("Only Super Admins can create Super Admin accounts".to_string()));
    }

    if req.role == "admin" && auth_user.role != "super_admin" {
        return Err(AppError::Forbidden("Only Super Admins can create Admin accounts".to_string()));
    }

    let password_hash = hash_password(&req.password)?;

    let id = sqlx::query_scalar!(
        r#"
        INSERT INTO users (email, full_name, password_hash, role)
        VALUES ($1, $2, $3, $4)
        RETURNING id
        "#,
        req.email.to_lowercase(),
        req.full_name,
        password_hash,
        req.role
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        if e.to_string().contains("unique") {
            AppError::Conflict("Email already exists".to_string())
        } else {
            AppError::Database(e)
        }
    })?;

    Ok(Json(serde_json::json!({ "id": id, "message": "User created successfully" })))
}

pub async fn update_user(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateUserRequest>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&auth_user, "users.update")?;

    if let Some(ref new_role) = req.role {
        if new_role == "super_admin" && auth_user.role != "super_admin" {
            return Err(AppError::Forbidden("Only Super Admins can assign Super Admin role".to_string()));
        }
    }

    sqlx::query!(
        r#"
        UPDATE users
        SET
            full_name = COALESCE($1, full_name),
            role = COALESCE($2, role),
            is_active = COALESCE($3, is_active),
            updated_at = NOW()
        WHERE id = $4
        "#,
        req.full_name,
        req.role,
        req.is_active,
        id
    )
    .execute(&state.pool)
    .await?;

    Ok(Json(serde_json::json!({ "message": "User updated" })))
}

pub async fn deactivate_user(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&auth_user, "users.deactivate")?;

    // Prevent deactivating self
    if id == auth_user.id {
        return Err(AppError::BusinessLogic("Cannot deactivate your own account".to_string()));
    }

    // Prevent removing last super admin
    let target = sqlx::query_scalar!("SELECT role FROM users WHERE id = $1", id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    if target == "super_admin" {
        let active_super_admins: i64 = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM users WHERE role = 'super_admin' AND is_active = true"
        )
        .fetch_one(&state.pool)
        .await?
        .unwrap_or(0);

        if active_super_admins <= 1 {
            return Err(AppError::BusinessLogic(
                "Cannot deactivate the last active Super Admin".to_string()
            ));
        }
    }

    sqlx::query!("UPDATE users SET is_active = false, updated_at = NOW() WHERE id = $1", id)
        .execute(&state.pool)
        .await?;

    // Revoke sessions
    sqlx::query!(
        "UPDATE refresh_tokens SET revoked_at = NOW() WHERE user_id = $1 AND revoked_at IS NULL",
        id
    )
    .execute(&state.pool)
    .await?;

    Ok(Json(serde_json::json!({ "message": "User deactivated and sessions revoked" })))
}

pub async fn get_permissions(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&auth_user, "roles.manage")?;

    let permissions: Vec<String> = sqlx::query_scalar!(
        r#"
        SELECT p.key FROM user_permission_overrides upo
        JOIN permissions p ON p.id = upo.permission_id
        WHERE upo.user_id = $1 AND upo.granted = true
        "#,
        id
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    Ok(Json(serde_json::json!({ "permissions": permissions })))
}

pub async fn update_permissions(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdatePermissionsRequest>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&auth_user, "roles.manage")?;

    // Clear existing overrides and set new ones
    let mut tx = state.pool.begin().await?;

    sqlx::query!("DELETE FROM user_permission_overrides WHERE user_id = $1", id)
        .execute(&mut *tx)
        .await?;

    for perm_key in &req.permissions {
        sqlx::query!(
            r#"
            INSERT INTO user_permission_overrides (user_id, permission_id, granted)
            SELECT $1, p.id, true FROM permissions p WHERE p.key = $2
            "#,
            id, perm_key
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(Json(serde_json::json!({ "message": "Permissions updated" })))
}
