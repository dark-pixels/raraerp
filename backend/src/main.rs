use std::net::SocketAddr;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod config;
mod common;
mod database;
mod errors;
mod middleware;
mod routes;
mod auth;
mod users;
mod settings;
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
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "rara_erp_backend=info,tower_http=info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting RARA Enterprise ERP Backend v{}", env!("CARGO_PKG_VERSION"));

    let config = AppConfig::from_env();
    auth::service::init(config.jwt_secret.clone());
    info!(port = config.port, env = %config.app_env, "Configuration loaded");

    // Lazy pool: the server starts listening immediately even if the database is slow to accept connections.
    let pool = match create_pool(&config.database_url, config.db_max_connections) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("FATAL: invalid DATABASE_URL: {}", e);
            std::process::exit(1);
        }
    };

    // Run migrations in the background so the readiness probe passes quickly.
    if !config.database_url.is_empty() {
        let migration_pool = pool.clone();
        tokio::spawn(async move {
            match sqlx::migrate!("./migrations").run(&migration_pool).await {
                Ok(_) => info!("Database migrations applied"),
                Err(e) => tracing::error!("Database migration failed: {}", e),
            }
        });
    }

    let app = routes::create_router(pool, config.clone());

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("FATAL: Failed to bind TCP listener on {}: {}", addr, e);
            std::process::exit(1);
        }
    };
    info!(addr = %addr, "Listening");

    if let Err(e) = axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        eprintln!("Server error: {}", e);
        std::process::exit(1);
    }

    info!("Server shut down gracefully");
}

async fn shutdown_signal() {
    use tokio::signal;
    let ctrl_c = async {
        let _ = signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("Received Ctrl+C signal"),
        _ = terminate => info!("Received SIGTERM signal"),
    }
}
