use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
use rand::rngs::OsRng;
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let password = std::env::args().nth(1).unwrap_or_else(|| "Admin@1234".to_string());
    let email = std::env::args().nth(2).unwrap_or_else(|| "admin@raraenterprise.com".to_string());

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2.hash_password(password.as_bytes(), &salt)
        .map_err(|e| format!("Hash error: {}", e))?
        .to_string();

    println!("Generated Argon2id hash for password: {}", password);
    println!("Hash: {}", hash);

    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL not set");
    let pool = PgPoolOptions::new().max_connections(2).connect(&db_url).await?;

    let res = sqlx::query!(
        "UPDATE users SET password_hash = $1 WHERE email = $2",
        hash, email
    ).execute(&pool).await?;

    println!("Updated {} user(s) with email {}", res.rows_affected(), email);
    println!("Password successfully set to: {}", password);

    Ok(())
}
