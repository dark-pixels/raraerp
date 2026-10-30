use std::sync::OnceLock;

use argon2::{password_hash::SaltString, Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use chrono::Utc;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand::rngs::OsRng;
use sha2::Digest;
use uuid::Uuid;

use crate::auth::models::{AuthenticatedUser, Claims};
use crate::database::DbPool;
use crate::errors::AppError;

static JWT_SECRET: OnceLock<String> = OnceLock::new();

/// Must be called once at startup with the resolved secret from `AppConfig`.
pub fn init(secret: String) {
    let _ = JWT_SECRET.set(secret);
}

fn jwt_secret() -> &'static str {
    JWT_SECRET.get_or_init(|| crate::config::AppConfig::from_env().jwt_secret)
}

pub const MIN_PASSWORD_LEN: usize = 8;

pub fn validate_password_strength(password: &str) -> Result<(), AppError> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(AppError::Validation(format!(
            "Password must be at least {} characters",
            MIN_PASSWORD_LEN
        )));
    }
    let has_letter = password.chars().any(|c| c.is_alphabetic());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    if !has_letter || !has_digit {
        return Err(AppError::Validation("Password must contain both letters and numbers".into()));
    }
    Ok(())
}

pub fn create_access_token(user_id: &str, email: &str, role: &str, expiry_seconds: u64) -> Result<String, AppError> {
    let now = Utc::now().timestamp() as usize;
    let claims = Claims {
        sub: user_id.to_string(),
        email: email.to_string(),
        role: role.to_string(),
        exp: now + expiry_seconds as usize,
        iat: now,
        jti: Uuid::new_v4().to_string(),
    };

    encode(&Header::default(), &claims, &EncodingKey::from_secret(jwt_secret().as_bytes()))
        .map_err(|e| AppError::Internal(format!("Failed to create token: {}", e)))
}

pub fn verify_access_token(token: &str) -> Result<Claims, AppError> {
    let validation = Validation::new(Algorithm::HS256);
    decode::<Claims>(token, &DecodingKey::from_secret(jwt_secret().as_bytes()), &validation)
        .map(|data| data.claims)
        .map_err(|e| match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => AppError::Unauthorized("Session expired".into()),
            _ => AppError::Unauthorized("Invalid session token".into()),
        })
}

pub fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(format!("Password hashing failed: {}", e)))
}

/// Returns false (rather than erroring) for malformed hashes so a corrupt
/// record behaves like a wrong password instead of a server error.
pub fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|parsed| Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
        .unwrap_or(false)
}

pub fn generate_refresh_token() -> String {
    use rand::Rng;
    let bytes: [u8; 32] = OsRng.gen();
    hex::encode(bytes)
}

pub fn hash_refresh_token(token: &str) -> String {
    hex::encode(sha2::Sha256::digest(token.as_bytes()))
}

/// Loads an active user together with their effective permissions:
/// role permissions, minus per-user revocations, plus per-user grants.
pub async fn load_authenticated_user(pool: &DbPool, user_id: Uuid) -> Result<Option<AuthenticatedUser>, AppError> {
    let user = sqlx::query_as::<_, AuthenticatedUser>(
        r#"
        SELECT
            u.id, u.email, u.full_name, u.role, u.is_active,
            COALESCE((
                SELECT json_agg(k ORDER BY k) FROM (
                    SELECT p.key AS k
                    FROM role_permissions rp
                    JOIN permissions p ON p.id = rp.permission_id
                    WHERE rp.role = u.role
                      AND NOT EXISTS (
                          SELECT 1 FROM user_permission_overrides o
                          WHERE o.user_id = u.id AND o.permission_id = p.id AND o.granted = false
                      )
                    UNION
                    SELECT p.key
                    FROM user_permission_overrides o
                    JOIN permissions p ON p.id = o.permission_id
                    WHERE o.user_id = u.id AND o.granted = true
                ) perms
            ), '[]'::json)::jsonb AS permissions
        FROM users u
        WHERE u.id = $1 AND u.is_active = true
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(user)
}
