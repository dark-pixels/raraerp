use axum::{extract::{State, Extension, Path}, Json};
use uuid::Uuid;
use crate::{auth::models::AuthenticatedUser, errors::{AppError, AppResult}, middleware::permission::require_permission, routes::AppState};

pub async fn verify_gstin(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "gst.verify")?;

    let gstin = body.get("gstin").and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Validation("gstin required".into()))?;

    // Validate GSTIN format (15 chars, starts with 2-digit state code)
    if gstin.len() != 15 {
        return Err(AppError::Validation("GSTIN must be exactly 15 characters".into()));
    }

    // Check cache (1 hour)
    let cached = sqlx::query!(
        r#"SELECT id, status, legal_name, trade_name, registration_type, state_jurisdiction,
                  registration_date, verified_at
           FROM gst_verification_records
           WHERE gstin = $1 AND verified_at > NOW() - INTERVAL '1 hour'
           ORDER BY verified_at DESC LIMIT 1"#,
        gstin
    ).fetch_optional(&state.pool).await?;

    if let Some(cached) = cached {
        return Ok(Json(serde_json::json!({
            "gstin": gstin,
            "status": cached.status,
            "legal_name": cached.legal_name,
            "trade_name": cached.trade_name,
            "registration_type": cached.registration_type,
            "state": cached.state_jurisdiction,
            "registration_date": cached.registration_date,
            "verified_at": cached.verified_at,
            "is_cached": true
        })));
    }

    // Call external GST API
    let api_base = std::env::var("GST_API_BASE_URL").unwrap_or_default();
    let api_key = std::env::var("GST_API_KEY").unwrap_or_default();

    if api_base.is_empty() || api_key.is_empty() {
        // Store as pending and return
        sqlx::query!(
            r#"INSERT INTO gst_verification_records (gstin, status, verified_by)
               VALUES ($1, 'pending', $2)
               ON CONFLICT DO NOTHING"#,
            gstin, user.id
        ).execute(&state.pool).await.ok();

        return Ok(Json(serde_json::json!({
            "gstin": gstin,
            "status": "unknown",
            "message": "GST API not configured. Please add GST_API_BASE_URL and GST_API_KEY to environment variables.",
            "is_cached": false
        })));
    }

    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/taxpayerDetails/{}", api_base, gstin))
        .header("apikey", &api_key)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await;

    match response {
        Ok(resp) if resp.status().is_success() => {
            let data: serde_json::Value = resp.json().await
                .map_err(|_| AppError::ExternalService("Failed to parse GST API response".into()))?;

            // Map response to our schema
            let status = map_gst_status(data.get("sts").and_then(|v| v.as_str()).unwrap_or(""));
            let legal_name = data.get("lgnm").and_then(|v| v.as_str());
            let trade_name = data.get("tradeNam").and_then(|v| v.as_str());
            let reg_type = data.get("dty").and_then(|v| v.as_str());
            let state_jurisdiction = data.get("stj").and_then(|v| v.as_str());

            sqlx::query!(
                r#"INSERT INTO gst_verification_records (gstin, status, legal_name, trade_name, registration_type, state_jurisdiction, verified_by)
                   VALUES ($1,$2,$3,$4,$5,$6,$7)"#,
                gstin, status, legal_name, trade_name, reg_type, state_jurisdiction, user.id
            ).execute(&state.pool).await.ok();

            Ok(Json(serde_json::json!({
                "gstin": gstin,
                "status": status,
                "legal_name": legal_name,
                "trade_name": trade_name,
                "registration_type": reg_type,
                "state": state_jurisdiction,
                "is_cached": false
            })))
        }
        Ok(resp) => {
            let status_code = resp.status().as_u16();
            Err(AppError::ExternalService(format!("GST API returned HTTP {}", status_code)))
        }
        Err(e) if e.is_timeout() => {
            Err(AppError::ExternalService("GST API request timed out".into()))
        }
        Err(e) => {
            Err(AppError::ExternalService(format!("GST API connection error: {}", e)))
        }
    }
}

pub async fn list_verifications(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "gst.verify")?;
    let rows = sqlx::query!(
        "SELECT id, gstin, status, legal_name, trade_name, verified_at FROM gst_verification_records ORDER BY verified_at DESC LIMIT 100"
    ).fetch_all(&state.pool).await?;

    let data: Vec<_> = rows.iter().map(|r| serde_json::json!({
        "id": r.id, "gstin": r.gstin, "status": r.status,
        "legal_name": r.legal_name, "trade_name": r.trade_name, "verified_at": r.verified_at
    })).collect();
    Ok(Json(serde_json::json!({"data": data})))
}

pub async fn get_verification(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "gst.verify")?;
    let row = sqlx::query!(
        "SELECT * FROM gst_verification_records WHERE id = $1", id
    ).fetch_optional(&state.pool).await?
     .ok_or_else(|| AppError::NotFound("Verification record not found".into()))?;
    Ok(Json(serde_json::json!({
        "id": row.id, "gstin": row.gstin, "status": row.status,
        "legal_name": row.legal_name, "trade_name": row.trade_name,
        "registration_type": row.registration_type,
        "state": row.state_jurisdiction,
        "verified_at": row.verified_at
    })))
}

fn map_gst_status(api_status: &str) -> &'static str {
    match api_status.to_lowercase().as_str() {
        "active" | "act" => "active",
        "cancelled" | "cnc" => "cancelled",
        "suspended" | "sus" => "suspended",
        _ => "unknown",
    }
}
