use axum::{extract::{State, Extension, Path, Query}, Json};
use uuid::Uuid;
use crate::{auth::models::AuthenticatedUser, errors::{AppError, AppResult}, middleware::permission::require_permission, routes::AppState};

pub async fn list_warehouses(
    State(state): State<AppState>,
    Extension(_): Extension<AuthenticatedUser>,
) -> AppResult<Json<serde_json::Value>> {
    let rows = sqlx::query!("SELECT id, name, code, address, is_active FROM warehouses ORDER BY name")
        .fetch_all(&state.pool).await?;
    let data: Vec<_> = rows.iter().map(|r| serde_json::json!({"id": r.id, "name": r.name, "code": r.code, "address": r.address, "is_active": r.is_active})).collect();
    Ok(Json(serde_json::json!({"data": data})))
}

pub async fn create_warehouse(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "settings.manage")?;
    let name = body.get("name").and_then(|v| v.as_str()).ok_or_else(|| AppError::Validation("name required".into()))?;
    let code = body.get("code").and_then(|v| v.as_str()).ok_or_else(|| AppError::Validation("code required".into()))?;
    let id = sqlx::query_scalar!(
        "INSERT INTO warehouses (name, code, address) VALUES ($1, $2, $3) RETURNING id",
        name, code, body.get("address").and_then(|v| v.as_str())
    ).fetch_one(&state.pool).await?;
    Ok(Json(serde_json::json!({"id": id})))
}

pub async fn list_racks(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(q): Query<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "inventory.read")?;
    let warehouse_id: Option<Uuid> = q.get("warehouse_id")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok());

    let rows = sqlx::query!(
        r#"SELECT rl.id, rl.rack_code, rl.shelf, rl.bin, rl.is_active,
                  w.name as warehouse_name, wz.name as "zone_name?"
           FROM rack_locations rl
           JOIN warehouses w ON w.id = rl.warehouse_id
           LEFT JOIN warehouse_zones wz ON wz.id = rl.zone_id
           WHERE ($1::uuid IS NULL OR rl.warehouse_id = $1)
           ORDER BY w.name, rl.rack_code
           LIMIT 200"#,
        warehouse_id
    ).fetch_all(&state.pool).await?;

    let data: Vec<_> = rows.iter().map(|r| serde_json::json!({
        "id": r.id, "rack_code": r.rack_code, "shelf": r.shelf, "bin": r.bin,
        "warehouse_name": r.warehouse_name, "zone_name": r.zone_name, "is_active": r.is_active
    })).collect();
    Ok(Json(serde_json::json!({"data": data})))
}

pub async fn create_rack(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "inventory.rack.assign")?;
    let warehouse_id: Uuid = body.get("warehouse_id").and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| AppError::Validation("warehouse_id required".into()))?;
    let rack_code = body.get("rack_code").and_then(|v| v.as_str()).ok_or_else(|| AppError::Validation("rack_code required".into()))?;

    let id = sqlx::query_scalar!(
        r#"INSERT INTO rack_locations (warehouse_id, rack_code, shelf, bin)
           VALUES ($1, $2, $3, $4) RETURNING id"#,
        warehouse_id, rack_code,
        body.get("shelf").and_then(|v| v.as_str()),
        body.get("bin").and_then(|v| v.as_str())
    ).fetch_one(&state.pool).await?;
    Ok(Json(serde_json::json!({"id": id})))
}

pub async fn rack_inventory(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "inventory.read")?;
    let rows = sqlx::query!(
        r#"SELECT ib.id, ib.on_hand, ib.available, ib.reserved,
                  p.name as product_name, p.sku, p.barcode
           FROM inventory_balances ib
           JOIN products p ON p.id = ib.product_id
           WHERE ib.location_id = $1"#, id
    ).fetch_all(&state.pool).await?;

    let data: Vec<_> = rows.iter().map(|r| serde_json::json!({
        "product_name": r.product_name, "sku": r.sku, "barcode": r.barcode,
        "on_hand": r.on_hand, "available": r.available, "reserved": r.reserved
    })).collect();
    Ok(Json(serde_json::json!({"data": data})))
}

pub async fn rack_transfer(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "inventory.transfer")?;

    let product_id: Uuid = body.get("product_id").and_then(|v| v.as_str()).and_then(|s| s.parse().ok())
        .ok_or_else(|| AppError::Validation("product_id required".into()))?;
    let from_id: Uuid = body.get("from_location_id").and_then(|v| v.as_str()).and_then(|s| s.parse().ok())
        .ok_or_else(|| AppError::Validation("from_location_id required".into()))?;
    let to_id: Uuid = body.get("to_location_id").and_then(|v| v.as_str()).and_then(|s| s.parse().ok())
        .ok_or_else(|| AppError::Validation("to_location_id required".into()))?;
    let qty: i32 = body.get("quantity").and_then(|v| v.as_i64()).map(|v| v as i32)
        .ok_or_else(|| AppError::Validation("quantity required".into()))?;

    if qty <= 0 { return Err(AppError::Validation("Quantity must be positive".into())); }

    let mut tx = state.pool.begin().await?;

    // Check source stock
    let src: Option<rust_decimal::Decimal> = sqlx::query_scalar!(
        r#"SELECT available FROM inventory_balances WHERE product_id = $1 AND location_id = $2 FOR UPDATE"#,
        product_id, from_id
    ).fetch_optional(&mut *tx).await?.flatten();

    let avail = src.unwrap_or(rust_decimal::Decimal::ZERO);

    if avail < rust_decimal::Decimal::from(qty) {
        return Err(AppError::BusinessLogic(format!("Insufficient stock. Available: {}", avail)));
    }

    // Decrease source
    sqlx::query!(
        "UPDATE inventory_balances SET on_hand = on_hand - $1 WHERE product_id = $2 AND location_id = $3",
        rust_decimal::Decimal::from(qty), product_id, from_id
    ).execute(&mut *tx).await?;

    // Increase destination (upsert)
    sqlx::query!(
        r#"INSERT INTO inventory_balances (product_id, location_id, on_hand)
           VALUES ($1, $2, $3)
           ON CONFLICT (product_id, location_id) DO UPDATE
           SET on_hand = inventory_balances.on_hand + $3"#,
        product_id, to_id, rust_decimal::Decimal::from(qty)
    ).execute(&mut *tx).await?;

    // Log movement
    sqlx::query!(
        r#"INSERT INTO stock_movements (product_id, quantity, source_location_id, destination_location_id, movement_type, user_id, reason)
           VALUES ($1, $2, $3, $4, 'rack_transfer', $5, $6)"#,
        product_id, rust_decimal::Decimal::from(qty), from_id, to_id, user.id,
        body.get("reason").and_then(|v| v.as_str()).unwrap_or("Rack transfer")
    ).execute(&mut *tx).await?;

    tx.commit().await?;

    Ok(Json(serde_json::json!({"message": "Transfer completed successfully"})))
}
