use std::net::SocketAddr;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod config;
mod database;
mod errors;
mod middleware;
mod routes;
mod auth;
mod users;
mod permissions;
mod inventory;
mod racks;
mod stock_movements;
mod purchases;
mod suppliers;
mod customers;
mod sales;
mod invoices;
mod payments;
mod finance;
mod gst;
mod cloudinary;
mod notifications;
mod reports;
mod audit;
mod jobs;

use config::AppConfig;
use database::create_pool;

#[tokio::main]
async fn main() {
    // Load environment
    dotenvy::dotenv().ok();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "rara_erp_backend=debug,tower_http=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting RARA Enterprise ERP Backend");

    // Load configuration safely
    let config = AppConfig::from_env().unwrap_or_else(|err| {
        eprintln!("Configuration warning: {}", err);
        AppConfig::default()
    });
    info!(port = config.port, env = %config.app_env, "Configuration loaded");

    // Create database pool (lazy, non-blocking)
    let pool = match create_pool(&config.database_url, config.db_max_connections) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Database pool init warning: {}", e);
            create_pool("postgres://localhost:5432/placeholder", 1).expect("Failed to initialize fallback pool")
        }
    };
    info!("Database pool initialized");

    // Run migrations in background so server starts listening instantly
    if !config.database_url.is_empty() && !config.database_url.contains("placeholder") {
        let migration_pool = pool.clone();
        tokio::spawn(async move {
            match sqlx::migrate!("./migrations").run(&migration_pool).await {
                Ok(_) => info!("Database migrations applied"),
                Err(e) => tracing::warn!("Database migrations notice (already applied or skipped): {}", e),
            }
        });
    }

    // Build application router
    let app = routes::create_router(pool.clone(), config.clone()).await;
    info!("Router configured");

    // Bind address
    let port = config.port;
    let addr: SocketAddr = format!("0.0.0.0:{}", port)
        .parse()
        .expect("Invalid bind address");

    info!(addr = %addr, "Listening");

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("FATAL: Failed to bind TCP listener on {}: {}", addr, e);
            std::process::exit(1);
        }
    };

    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Server error");

    info!("Server shut down gracefully");
}

async fn shutdown_signal() {
    use tokio::signal;
    let ctrl_c = async {
        signal::ctrl_c().await.expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("Received Ctrl+C signal"),
        _ = terminate => info!("Received SIGTERM signal"),
    }
}
