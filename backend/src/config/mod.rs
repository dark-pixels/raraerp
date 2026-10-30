use std::env;

/// Development-only signing key. Never used when APP_ENV=production.
const DEV_JWT_SECRET: &str = "rara-erp-local-development-secret-do-not-use-in-production-0000000000";

#[derive(Debug, Clone)]
pub struct AppConfig {
    // Server
    pub port: u16,
    pub app_env: String,
    /// Comma-separated list of allowed origins, or "*".
    pub frontend_origin: String,

    // Database
    pub database_url: String,
    pub db_max_connections: u32,

    // JWT
    pub jwt_secret: String,
    pub jwt_expiry_seconds: u64,
    pub refresh_token_expiry_days: i64,

    // Cloudinary
    pub cloudinary_cloud_name: String,
    pub cloudinary_api_key: String,
    pub cloudinary_api_secret: String,

    // Razorpay
    pub razorpay_key_id: String,
    pub razorpay_key_secret: String,
    pub razorpay_webhook_secret: String,

    // GST API
    pub gst_api_base_url: String,
    pub gst_api_key: String,

    // UPI
    pub upi_merchant_id: String,
    pub upi_merchant_name: String,

    /// Shared secret for the scheduled-jobs endpoint (Vercel Cron sends it as a Bearer token).
    pub cron_secret: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

fn var(key: &str) -> String {
    env::var(key).map(|v| v.trim().to_string()).unwrap_or_default()
}

fn var_or(key: &str, default: &str) -> String {
    let v = var(key);
    if v.is_empty() { default.to_string() } else { v }
}

fn parse_or<T: std::str::FromStr>(key: &str, default: T) -> T {
    env::var(key).ok().and_then(|v| v.trim().parse().ok()).unwrap_or(default)
}

/// Picks the JWT signing secret. In production a missing or short secret is a
/// misconfiguration: rather than fall back to a publicly known key (which would
/// let anyone forge tokens), a random per-process key is generated. Sessions
/// then do not survive restarts, which makes the problem visible.
fn resolve_jwt_secret(app_env: &str) -> String {
    let secret = var("JWT_SECRET");
    if secret.len() >= 32 {
        return secret;
    }
    if app_env == "production" {
        tracing::error!(
            "JWT_SECRET is missing or shorter than 32 characters. Using a random per-process key; \
             users will be signed out whenever the server restarts. Set JWT_SECRET to fix this."
        );
        use rand::Rng;
        let mut bytes = [0u8; 48];
        rand::thread_rng().fill(&mut bytes[..]);
        return hex::encode(bytes);
    }
    if !secret.is_empty() {
        tracing::warn!("JWT_SECRET is shorter than 32 characters; use a longer secret in production");
        return secret;
    }
    tracing::warn!("JWT_SECRET not set; using the development key");
    DEV_JWT_SECRET.to_string()
}

impl AppConfig {
    pub fn from_env() -> Self {
        let app_env = var_or("APP_ENV", "production").to_lowercase();
        let database_url = var("DATABASE_URL");
        if database_url.is_empty() {
            tracing::error!("DATABASE_URL is not set; all database operations will fail");
        }

        Self {
            port: parse_or("PORT", 8080),
            jwt_secret: resolve_jwt_secret(&app_env),
            app_env,
            frontend_origin: var_or("FRONTEND_ORIGIN", "*"),

            database_url,
            db_max_connections: parse_or("DB_MAX_CONNECTIONS", 10),

            jwt_expiry_seconds: parse_or("JWT_EXPIRY_SECONDS", 3600),
            refresh_token_expiry_days: parse_or("REFRESH_TOKEN_EXPIRY_DAYS", 30),

            cloudinary_cloud_name: var("CLOUDINARY_CLOUD_NAME"),
            cloudinary_api_key: var("CLOUDINARY_API_KEY"),
            cloudinary_api_secret: var("CLOUDINARY_API_SECRET"),

            razorpay_key_id: var("RAZORPAY_KEY_ID"),
            razorpay_key_secret: var("RAZORPAY_KEY_SECRET"),
            razorpay_webhook_secret: var("RAZORPAY_WEBHOOK_SECRET"),

            gst_api_base_url: var("GST_API_BASE_URL"),
            gst_api_key: var("GST_API_KEY"),

            upi_merchant_id: var("UPI_MERCHANT_ID"),
            upi_merchant_name: var_or("UPI_MERCHANT_NAME", "RARA Enterprise"),

            cron_secret: var("CRON_SECRET"),
        }
    }

    pub fn allowed_origins(&self) -> Vec<String> {
        self.frontend_origin
            .split(',')
            .map(|o| o.trim().trim_end_matches('/').to_string())
            .filter(|o| !o.is_empty())
            .collect()
    }
}
