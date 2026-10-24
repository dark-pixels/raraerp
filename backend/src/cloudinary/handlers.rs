use axum::{extract::{State, Extension, Path, Multipart}, Json};
use uuid::Uuid;
use base64::{Engine as _, engine::general_purpose};
use crate::{auth::models::AuthenticatedUser, errors::{AppError, AppResult}, middleware::permission::require_permission, routes::AppState};

pub async fn upload_image(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    mut multipart: Multipart,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "inventory.update")?;

    let cloud_name = std::env::var("CLOUDINARY_CLOUD_NAME").unwrap_or_default();
    let api_key = std::env::var("CLOUDINARY_API_KEY").unwrap_or_default();
    let api_secret = std::env::var("CLOUDINARY_API_SECRET").unwrap_or_default();

    if cloud_name.is_empty() {
        return Err(AppError::ExternalService("Cloudinary not configured".into()));
    }

    let mut file_bytes: Vec<u8> = Vec::new();
    let mut mime_type = String::new();
    let mut product_id: Option<Uuid> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| AppError::Validation(e.to_string()))? {
        match field.name() {
            Some("file") => {
                mime_type = field.content_type().unwrap_or("image/jpeg").to_string();
                file_bytes = field.bytes().await.map_err(|e| AppError::Validation(e.to_string()))?.to_vec();
            }
            Some("product_id") => {
                product_id = field.text().await.ok().and_then(|s| s.parse().ok());
            }
            _ => {}
        }
    }

    // Validate content
    if file_bytes.is_empty() { return Err(AppError::Validation("No file provided".into())); }
    if file_bytes.len() > 10 * 1024 * 1024 { return Err(AppError::Validation("File too large (max 10MB)".into())); }

    // Validate mime type
    if !["image/jpeg", "image/png", "image/webp"].contains(&mime_type.as_str()) {
        return Err(AppError::Validation("Only JPEG, PNG, and WebP images are allowed".into()));
    }

    // Generate Cloudinary signature
    let timestamp = chrono::Utc::now().timestamp().to_string();
    let folder = "rara-erp/products";
    let to_sign = format!("folder={}&timestamp={}{}", folder, timestamp, api_secret);
    let signature = sha256_hex(&to_sign);

    // Upload to Cloudinary
    let form = reqwest::multipart::Form::new()
        .text("api_key", api_key.clone())
        .text("timestamp", timestamp.clone())
        .text("signature", signature)
        .text("folder", folder.to_string())
        .part("file", reqwest::multipart::Part::bytes(file_bytes).mime_str(&mime_type).unwrap());

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("https://api.cloudinary.com/v1_1/{}/image/upload", cloud_name))
        .multipart(form)
        .send()
        .await
        .map_err(|e| AppError::ExternalService(format!("Cloudinary upload failed: {}", e)))?;

    if !resp.status().is_success() {
        let err_body: serde_json::Value = resp.json().await.unwrap_or_default();
        return Err(AppError::ExternalService(format!("Cloudinary error: {}", err_body)));
    }

    let cl_resp: serde_json::Value = resp.json().await
        .map_err(|_| AppError::ExternalService("Invalid Cloudinary response".into()))?;

    let public_id = cl_resp.get("public_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let url = cl_resp.get("secure_url").and_then(|v| v.as_str()).unwrap_or("").to_string();

    // Store in DB
    sqlx::query!(
        "INSERT INTO product_images (product_id, public_id, url, uploaded_by) VALUES ($1,$2,$3,$4)",
        product_id, public_id, url, user.id
    ).execute(&state.pool).await.ok();

    // Update product image_url if product_id provided
    if let Some(pid) = product_id {
        sqlx::query!(
            "UPDATE products SET image_url = $1 WHERE id = $2",
            url, pid
        ).execute(&state.pool).await.ok();
    }

    Ok(Json(serde_json::json!({
        "public_id": public_id,
        "url": url,
        "message": "Image uploaded successfully"
    })))
}

pub async fn delete_image(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "inventory.update")?;

    let img = sqlx::query!("SELECT public_id FROM product_images WHERE id = $1", id)
        .fetch_optional(&state.pool).await?
        .ok_or_else(|| AppError::NotFound("Image not found".into()))?;

    let cloud_name = std::env::var("CLOUDINARY_CLOUD_NAME").unwrap_or_default();
    let api_key = std::env::var("CLOUDINARY_API_KEY").unwrap_or_default();
    let api_secret = std::env::var("CLOUDINARY_API_SECRET").unwrap_or_default();

    if !cloud_name.is_empty() {
        let timestamp = chrono::Utc::now().timestamp().to_string();
        let to_sign = format!("public_id={}&timestamp={}{}", img.public_id, timestamp, api_secret);
        let signature = sha256_hex(&to_sign);

        let client = reqwest::Client::new();
        let _ = client
            .post(format!("https://api.cloudinary.com/v1_1/{}/image/destroy", cloud_name))
            .form(&[
                ("public_id", img.public_id.as_str()),
                ("api_key", &api_key),
                ("timestamp", &timestamp),
                ("signature", &signature),
            ])
            .send().await;
    }

    sqlx::query!("DELETE FROM product_images WHERE id = $1", id).execute(&state.pool).await?;

    Ok(Json(serde_json::json!({"message": "Image deleted"})))
}

fn sha256_hex(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}
