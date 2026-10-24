use tracing::{info, error, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod config;
mod database;
mod errors;
mod jobs;

use config::AppConfig;
use database::create_pool;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "rara_erp_worker=info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting RARA Enterprise ERP Background Worker");

    let config = AppConfig::from_env().expect("Failed to load configuration");
    let pool = create_pool(&config.database_url, 5)
        .await
        .expect("Failed to create database pool");

    info!("Worker database pool ready");

    // Start the background job scheduler
    jobs::start_scheduler(pool).await;
}
