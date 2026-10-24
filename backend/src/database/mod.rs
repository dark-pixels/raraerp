use sqlx::{Pool, Postgres, postgres::PgPoolOptions};
use std::time::Duration;
use tracing::info;

pub type DbPool = Pool<Postgres>;

pub fn create_pool(database_url: &str, max_connections: u32) -> Result<DbPool, sqlx::Error> {
    let url = if database_url.trim().is_empty() {
        "postgres://postgres:postgres@localhost:5432/rara"
    } else {
        database_url
    };

    info!("Initializing PostgreSQL lazy pool (max_connections={})", max_connections);
    let pool = PgPoolOptions::new()
        .max_connections(max_connections)
        .min_connections(0)
        .acquire_timeout(Duration::from_secs(10))
        .idle_timeout(Duration::from_secs(600))
        .max_lifetime(Duration::from_secs(1800))
        .connect_lazy(url)?;

    Ok(pool)
}
