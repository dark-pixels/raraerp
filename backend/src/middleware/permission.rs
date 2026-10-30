use crate::auth::models::AuthenticatedUser;
use crate::errors::AppError;

pub fn has_permission(user: &AuthenticatedUser, permission: &str) -> bool {
    user.role == "super_admin" || user.permissions.iter().any(|p| p == permission)
}

pub fn require_permission(user: &AuthenticatedUser, permission: &str) -> Result<(), AppError> {
    if has_permission(user, permission) {
        Ok(())
    } else {
        Err(AppError::Forbidden(permission.to_string()))
    }
}
