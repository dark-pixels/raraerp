use crate::auth::models::AuthenticatedUser;
use crate::errors::AppError;

pub fn check_permission(user: &AuthenticatedUser, permission: &str) -> Result<(), AppError> {
    if user.role == "super_admin" {
        return Ok(());
    }
    if user.permissions.contains(&permission.to_string()) {
        return Ok(());
    }
    Err(AppError::Forbidden(permission.to_string()))
}

pub fn require_permission(user: &AuthenticatedUser, permission: &str) -> Result<(), AppError> {
    check_permission(user, permission)
}

pub fn require_any_permission(
    user: &AuthenticatedUser,
    permissions: &[&str],
) -> Result<(), AppError> {
    if user.role == "super_admin" {
        return Ok(());
    }
    for perm in permissions {
        if user.permissions.contains(&perm.to_string()) {
            return Ok(());
        }
    }
    Err(AppError::Forbidden(permissions.join(" or ")))
}
