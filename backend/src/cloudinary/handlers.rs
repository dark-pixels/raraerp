use axum::{
    extract::{Extension, Multipart, Path, State},
    Json,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    auth::models::AuthenticatedUser,
    errors::{AppError, AppResult},
    middleware::permission::require_permission,
    routes::AppState,
};

const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;
const ALLOWED_TYPES: &[&str] = &["image/jpeg", "image/png", "image/webp"];
const FOLDER: &str = "rara-erp/products";

/// Cloudinary signature: SHA-1 over the alphabetically sorted params followed by the API secret.
fn sign(params: &[(&str, &str)], api_secret: &str) -> String {
    use sha1::{Digest, Sha1};
    let mut sorted: Vec<_> = params.to_vec();
    sorted.sort_by(|a, b| a.0.cmp(b.0));
    let to_sign = sorted.iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<_>>().join("&");
    hex::encode(Sha1::digest(format!("{}{}", to_sign, api_secret).as_bytes()))
}

/// Detects the real image type from magic bytes instead of trusting the client.
fn sniff_image_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("image/png")
    } else if bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

pub async fn upload_image(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    mut multipart: Multipart,
) -> AppResult<Json<Value>> {
    require_permission(&user, "inventory.update")?;
    let cfg = &state.config;
    if cfg.cloudinary_cloud_name.is_empty() || cfg.cloudinary_api_key.is_empty() || cfg.cloudinary_api_secret.is_empty() {
        return Err(AppError::ExternalService(
            "Image uploads are not configured. Set CLOUDINARY_CLOUD_NAME, CLOUDINARY_API_KEY and CLOUDINARY_API_SECRET.".into(),
        ));
    }

    let mut file_bytes: Vec<u8> = Vec::new();
    let mut product_id: Option<Uuid> = None;
    while let Some(field) = multipart.next_field().await.map_err(|e| AppError::Validation(e.to_string()))? {
        match field.name() {
            Some("file") => file_bytes = field.bytes().await.map_err(|e| AppError::Validation(e.to_string()))?.to_vec(),
            Some("product_id") => product_id = field.text().await.ok().and_then(|s| s.trim().parse().ok()),
            _ => {}
        }
    }

    if file_bytes.is_empty() {
        return Err(AppError::Validation("Choose an image to upload".into()));
    }
    if file_bytes.len() > MAX_IMAGE_BYTES {
        return Err(AppError::Validation("Image is too large (max 10 MB)".into()));
    }
    let mime_type = sniff_image_type(&file_bytes)
        .filter(|m| ALLOWED_TYPES.contains(m))
        .ok_or_else(|| AppError::Validation("Only JPEG, PNG and WebP images are allowed".into()))?;

    let timestamp = chrono::Utc::now().timestamp().to_string();
    let signature = sign(&[("folder", FOLDER), ("timestamp", &timestamp)], &cfg.cloudinary_api_secret);

    let part = reqwest::multipart::Part::bytes(file_bytes)
        .file_name("upload")
        .mime_str(mime_type)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let form = reqwest::multipart::Form::new()
        .text("api_key", cfg.cloudinary_api_key.clone())
        .text("timestamp", timestamp)
        .text("signature", signature)
        .text("folder", FOLDER)
        .part("file", part);

    let resp = reqwest::Client::new()
        .post(format!("https://api.cloudinary.com/v1_1/{}/image/upload", cfg.cloudinary_cloud_name))
        .timeout(std::time::Duration::from_secs(60))
        .multipart(form)
        .send()
        .await
        .map_err(|e| AppError::ExternalService(format!("Image upload failed: {}", e)))?;

    if !resp.status().is_success() {
        let err_body: Value = resp.json().await.unwrap_or_default();
        let msg = err_body.pointer("/error/message").and_then(|v| v.as_str()).unwrap_or("upload rejected");
        return Err(AppError::ExternalService(format!("Cloudinary: {}", msg)));
    }

    let cl_resp: Value = resp.json().await.map_err(|_| AppError::ExternalService("Invalid Cloudinary response".into()))?;
    let public_id = cl_resp.get("public_id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
    let url = cl_resp.get("secure_url").and_then(|v| v.as_str()).unwrap_or_default().to_string();
    if url.is_empty() {
        return Err(AppError::ExternalService("Cloudinary did not return an image URL".into()));
    }

    let mut tx = state.pool.begin().await?;
    if let Some(pid) = product_id {
        sqlx::query!("UPDATE product_images SET is_primary = false WHERE product_id = $1", pid)
            .execute(&mut *tx)
            .await?;
        sqlx::query!("UPDATE products SET image_url = $1, updated_at = NOW() WHERE id = $2", url, pid)
            .execute(&mut *tx)
            .await?;
    }
    let image_id = sqlx::query_scalar!(
        "INSERT INTO product_images (product_id, public_id, url, uploaded_by) VALUES ($1,$2,$3,$4) RETURNING id",
        product_id, public_id, url, user.id
    )
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(Json(json!({ "id": image_id, "public_id": public_id, "url": url, "message": "Image uploaded" })))
}

pub async fn delete_image(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "inventory.update")?;
    let cfg = &state.config;

    let img = sqlx::query!("SELECT public_id, product_id, url FROM product_images WHERE id = $1", id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Image not found".into()))?;

    if !cfg.cloudinary_cloud_name.is_empty() && !cfg.cloudinary_api_secret.is_empty() {
        let timestamp = chrono::Utc::now().timestamp().to_string();
        let signature = sign(&[("public_id", &img.public_id), ("timestamp", &timestamp)], &cfg.cloudinary_api_secret);
        let result = reqwest::Client::new()
            .post(format!("https://api.cloudinary.com/v1_1/{}/image/destroy", cfg.cloudinary_cloud_name))
            .timeout(std::time::Duration::from_secs(20))
            .form(&[
                ("public_id", img.public_id.as_str()),
                ("api_key", cfg.cloudinary_api_key.as_str()),
                ("timestamp", timestamp.as_str()),
                ("signature", signature.as_str()),
            ])
            .send()
            .await;
        if let Err(e) = result {
            tracing::warn!("Cloudinary delete failed for {}: {}", img.public_id, e);
        }
    }

    let mut tx = state.pool.begin().await?;
    sqlx::query!("DELETE FROM product_images WHERE id = $1", id).execute(&mut *tx).await?;
    if let Some(pid) = img.product_id {
        sqlx::query!("UPDATE products SET image_url = NULL WHERE id = $1 AND image_url = $2", pid, img.url)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;

    Ok(Json(json!({"message": "Image deleted"})))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_matches_cloudinary_docs_example() {
        // Example from Cloudinary's "Generating authentication signatures" guide.
        let sig = sign(&[("timestamp", "1315060510"), ("public_id", "sample_image"), ("eager", "w_400,h_300,c_pad|w_260,h_200,c_crop")], "abcd");
        assert_eq!(sig, "bfd09f95f331f558cbd1320e67aa8d488770583e");
    }

    #[test]
    fn sniffing() {
        assert_eq!(sniff_image_type(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("image/jpeg"));
        assert_eq!(sniff_image_type(b"\x89PNG\r\n"), Some("image/png"));
        assert_eq!(sniff_image_type(b"GIF89a"), None);
    }
}
