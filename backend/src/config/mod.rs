use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    // Server
    pub port: u16,
    pub app_env: String,
    pub frontend_origin: String,

    // Database
    pub database_url: String,
    pub db_max_connections: u32,

    // JWT
    pub jwt_secret: String,
    pub jwt_expiry_seconds: u64,
    pub refresh_token_expiry_days: u64,

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

    // Timezone
    pub timezone: String,

    // UPI
    pub upi_merchant_id: String,
    pub upi_merchant_name: String,

    // Notifications
    pub smtp_host: Option<String>,
    pub smtp_port: Option<u16>,
    pub smtp_username: Option<String>,
    pub smtp_password: Option<String>,
    pub smtp_from: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            app_env: "production".to_string(),
            frontend_origin: "*".to_string(),
            database_url: String::new(),
            db_max_connections: 10,
            jwt_secret: "rara-erp-secret-key-at-least-64-characters-for-secure-signing-token-2026-prod".to_string(),
            jwt_expiry_seconds: 3600,
            refresh_token_expiry_days: 30,
            cloudinary_cloud_name: String::new(),
            cloudinary_api_key: String::new(),
            cloudinary_api_secret: String::new(),
            razorpay_key_id: String::new(),
            razorpay_key_secret: String::new(),
            razorpay_webhook_secret: String::new(),
            gst_api_base_url: "https://api.gst-placeholder.example.com".to_string(),
            gst_api_key: String::new(),
            timezone: "Asia/Kolkata".to_string(),
            upi_merchant_id: String::new(),
            upi_merchant_name: "RARA Enterprise".to_string(),
            smtp_host: None,
            smtp_port: Some(587),
            smtp_username: None,
            smtp_password: None,
            smtp_from: None,
        }
    }
}

impl AppConfig {
    pub fn from_env() -> Result<Self, String> {
        let port: u16 = env::var("PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse()
            .unwrap_or(8080);

        let database_url = env::var("DATABASE_URL").unwrap_or_default();
        if database_url.is_empty() {
            eprintln!("WARNING: DATABASE_URL environment variable is not set! Set it in Vercel Project Settings.");
        }

        let jwt_secret = env::var("JWT_SECRET").unwrap_or_else(|_| {
            "rara-erp-secret-key-at-least-64-characters-for-secure-signing-token-2026-prod".to_string()
        });

        let frontend_origin = env::var("FRONTEND_ORIGIN").unwrap_or_else(|_| "*".to_string());

        Ok(Self {
            port,
            app_env: env::var("APP_ENV").unwrap_or_else(|_| "production".to_string()),
            frontend_origin,

            database_url,
            db_max_connections: env::var("DB_MAX_CONNECTIONS")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .unwrap_or(10),

            jwt_secret,
            jwt_expiry_seconds: env::var("JWT_EXPIRY_SECONDS")
                .unwrap_or_else(|_| "3600".to_string())
                .parse()
                .unwrap_or(3600),
            refresh_token_expiry_days: env::var("REFRESH_TOKEN_EXPIRY_DAYS")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .unwrap_or(30),

            cloudinary_cloud_name: env::var("CLOUDINARY_CLOUD_NAME")
                .unwrap_or_default(),
            cloudinary_api_key: env::var("CLOUDINARY_API_KEY")
                .unwrap_or_default(),
            cloudinary_api_secret: env::var("CLOUDINARY_API_SECRET")
                .unwrap_or_default(),

            razorpay_key_id: env::var("RAZORPAY_KEY_ID")
                .unwrap_or_default(),
            razorpay_key_secret: env::var("RAZORPAY_KEY_SECRET")
                .unwrap_or_default(),
            razorpay_webhook_secret: env::var("RAZORPAY_WEBHOOK_SECRET")
                .unwrap_or_default(),

            gst_api_base_url: env::var("GST_API_BASE_URL")
                .unwrap_or_else(|_| "https://api.gst-placeholder.example.com".to_string()),
            gst_api_key: env::var("GST_API_KEY")
                .unwrap_or_default(),

            timezone: env::var("TZ").unwrap_or_else(|_| "Asia/Kolkata".to_string()),

            upi_merchant_id: env::var("UPI_MERCHANT_ID").unwrap_or_default(),
            upi_merchant_name: env::var("UPI_MERCHANT_NAME")
                .unwrap_or_else(|_| "RARA Enterprise".to_string()),

            smtp_host: env::var("SMTP_HOST").ok(),
            smtp_port: env::var("SMTP_PORT").ok().and_then(|p| p.parse().ok()),
            smtp_username: env::var("SMTP_USERNAME").ok(),
            smtp_password: env::var("SMTP_PASSWORD").ok(),
            smtp_from: env::var("SMTP_FROM").ok(),
        })
    }

    pub fn is_production(&self) -> bool {
        self.app_env == "production"
    }
}
