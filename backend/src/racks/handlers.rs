use axum::{
    extract::{Extension, Path, Query, State},
    Json,
};
use rust_decimal::Decimal;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    audit::service::log_action,
    auth::models::AuthenticatedUser,
    common::{dec_field, required_text, text, uuid, ListQuery},
    errors::{AppError, AppResult},
    inventory::stock,
    middleware::permission::require_permission,
    routes::AppState,
};

pub async fn list_warehouses(
    State(state): State<AppState>,
    Extension(_): Extension<AuthenticatedUser>,
) -> AppResult<Json<Value>> {
    let rows = sqlx::query!(
        r#"SELECT w.id, w.name, w.code, w.address, w.is_active,
                  (SELECT COUNT(*) FROM rack_locations rl WHERE rl.warehouse_id = w.id) AS "rack_count!"
           FROM warehouses w ORDER BY w.name"#
    )
    .fetch_all(&state.pool)
    .await?;
    let data: Vec<Value> = rows
        .iter()
        .map(|r| json!({"id": r.id, "name": r.name, "code": r.code, "address": r.address, "is_active": r.is_active, "rack_count": r.rack_count}))
        .collect();
    Ok(Json(json!({"data": data})))
}

pub async fn create_warehouse(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(body): Json<Value>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "settings.manage")?;
    let name = required_text(&body, "name")?;
    let code = required_text(&body, "code")?.to_uppercase();
    let id = sqlx::query_scalar!(
        "INSERT INTO warehouses (name, code, address) VALUES ($1, $2, $3) RETURNING id",
        name, code, text(body.get("address"))
    )
    .fetch_one(&state.pool)
    .await?;
    log_action(&state.pool, user.id, "warehouse_created", "warehouse", Some(&id.to_string()), Some(json!({"name": name, "code": code}))).await;
    Ok(Json(json!({"id": id})))
}

pub async fn list_racks(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "inventory.read")?;
    let rows = sqlx::query!(
        r#"SELECT rl.id, rl.warehouse_id, rl.rack_code, rl.shelf, rl.bin, rl.is_active,
                  w.name AS warehouse_name, wz.name AS "zone_name?",
                  (SELECT COUNT(*) FROM inventory_balances ib WHERE ib.location_id = rl.id AND ib.on_hand > 0) AS "product_count!",
                  (SELECT COALESCE(SUM(ib.on_hand), 0) FROM inventory_balances ib WHERE ib.location_id = rl.id) AS "total_units!"
           FROM rack_locations rl
           JOIN warehouses w ON w.id = rl.warehouse_id
           LEFT JOIN warehouse_zones wz ON wz.id = rl.zone_id
           WHERE ($1::uuid IS NULL OR rl.warehouse_id = $1)
             AND ($2::text IS NULL OR rl.rack_code ILIKE '%' || $2 || '%')
           ORDER BY w.name, rl.rack_code, rl.shelf, rl.bin
           LIMIT 1000"#,
        q.warehouse_id, q.search()
    )
    .fetch_all(&state.pool)
    .await?;

    let data: Vec<Value> = rows
        .iter()
        .map(|r| {
            let label = [Some(r.rack_code.as_str()), r.shelf.as_deref(), r.bin.as_deref()]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join("-");
            json!({
                "id": r.id, "warehouse_id": r.warehouse_id, "rack_code": r.rack_code, "shelf": r.shelf, "bin": r.bin,
                "label": format!("{} / {}", r.warehouse_name, label),
                "warehouse_name": r.warehouse_name, "zone_name": r.zone_name, "is_active": r.is_active,
                "product_count": r.product_count, "total_units": r.total_units
            })
        })
        .collect();
    Ok(Json(json!({"data": data})))
}

pub async fn create_rack(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(body): Json<Value>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "inventory.rack.assign")?;
    let warehouse_id = uuid(body.get("warehouse_id")).ok_or_else(|| AppError::Validation("Select a warehouse".into()))?;
    let rack_code = required_text(&body, "rack_code")?.to_uppercase();
    let shelf = text(body.get("shelf"));
    let bin = text(body.get("bin"));

    // The unique constraint treats NULL shelf/bin as distinct, so check explicitly.
    let exists: bool = sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM rack_locations WHERE warehouse_id = $1 AND rack_code = $2
                         AND shelf IS NOT DISTINCT FROM $3 AND bin IS NOT DISTINCT FROM $4) AS "e!""#,
        warehouse_id, rack_code, shelf, bin
    )
    .fetch_one(&state.pool)
    .await?;
    if exists {
        return Err(AppError::Conflict("This rack location already exists".into()));
    }

    let id = sqlx::query_scalar!(
        "INSERT INTO rack_locations (warehouse_id, rack_code, shelf, bin) VALUES ($1, $2, $3, $4) RETURNING id",
        warehouse_id, rack_code, shelf, bin
    )
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(json!({"id": id})))
}

pub async fn rack_inventory(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "inventory.read")?;
    let rows = sqlx::query!(
        r#"SELECT ib.product_id, ib.on_hand, ib.available, ib.reserved, p.name AS product_name, p.sku, p.barcode
           FROM inventory_balances ib
           JOIN products p ON p.id = ib.product_id
           WHERE ib.location_id = $1 AND ib.on_hand > 0
           ORDER BY p.name"#,
        id
    )
    .fetch_all(&state.pool)
    .await?;

    let data: Vec<Value> = rows
        .iter()
        .map(|r| json!({
            "product_id": r.product_id, "product_name": r.product_name, "sku": r.sku, "barcode": r.barcode,
            "on_hand": r.on_hand, "available": r.available, "reserved": r.reserved
        }))
        .collect();
    Ok(Json(json!({"data": data})))
}

pub async fn rack_transfer(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(body): Json<Value>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "inventory.transfer")?;

    let product_id = uuid(body.get("product_id")).ok_or_else(|| AppError::Validation("Select a product".into()))?;
    let from_id = uuid(body.get("from_location_id")).ok_or_else(|| AppError::Validation("Select the source rack".into()))?;
    let to_id = uuid(body.get("to_location_id")).ok_or_else(|| AppError::Validation("Select the destination rack".into()))?;
    let qty = dec_field(&body, "quantity")?.ok_or_else(|| AppError::Validation("Quantity is required".into()))?;
    if qty <= Decimal::ZERO {
        return Err(AppError::Validation("Quantity must be greater than zero".into()));
    }
    if from_id == to_id {
        return Err(AppError::Validation("Source and destination racks must be different".into()));
    }
    let reason = text(body.get("reason")).unwrap_or_else(|| "Rack transfer".into());

    let mut tx = state.pool.begin().await?;
    stock::resolve_location(&mut tx, Some(to_id)).await?;
    stock::transfer_stock(&mut tx, product_id, from_id, to_id, qty, user.id, &reason).await?;
    tx.commit().await?;

    log_action(&state.pool, user.id, "stock_transferred", "product", Some(&product_id.to_string()),
        Some(json!({"from": from_id, "to": to_id, "quantity": qty}))).await;
    Ok(Json(json!({"message": "Transfer completed"})))
}
