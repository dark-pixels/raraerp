use serde::{Deserialize, Serialize};
use uuid::Uuid;
use sqlx::FromRow;


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,  // user_id
    pub email: String,
    pub role: String,
    pub exp: usize,
    pub iat: usize,
    pub jti: String,  // JWT ID for revocation
}

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct AuthenticatedUser {
    pub id: Uuid,
    pub email: String,
    pub full_name: String,
    pub role: String,
    pub is_active: bool,
    #[sqlx(json)]
    pub permissions: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
    pub user: UserInfo,
}

#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub id: String,
    pub email: String,
    pub full_name: String,
    pub role: String,
    pub permissions: Vec<String>,
    pub is_active: bool,
}

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, Serialize)]
pub struct RefreshResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

impl From<&AuthenticatedUser> for UserInfo {
    fn from(u: &AuthenticatedUser) -> Self {
        Self {
            id: u.id.to_string(),
            email: u.email.clone(),
            full_name: u.full_name.clone(),
            role: u.role.clone(),
            permissions: u.permissions.clone(),
            is_active: u.is_active,
        }
    }
}
