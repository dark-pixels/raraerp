//! Standalone background worker for long-running hosts (VPS, Docker).
//! On serverless hosts, call `GET /api/v1/cron/run` on a schedule instead.

use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

// The worker shares these modules with the server but uses only part of them.
#[allow(dead_code)]
mod common;
#[allow(dead_code)]
mod config;
mod database;
#[allow(dead_code)]
mod errors;
mod jobs;

use config::AppConfig;
use database::create_pool;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "worker=info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting RARA Enterprise ERP background worker");

    let config = AppConfig::from_env();
    let pool = match create_pool(&config.database_url, 5) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("FATAL: invalid DATABASE_URL: {}", e);
            std::process::exit(1);
        }
    };

    jobs::start_scheduler(pool).await;
}
