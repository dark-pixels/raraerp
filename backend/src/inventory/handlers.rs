use axum::{extract::{State, Extension, Path, Query, Multipart}, Json, http::StatusCode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use rust_decimal::Decimal;
use crate::{auth::models::AuthenticatedUser, errors::{AppError, AppResult}, middleware::permission::require_permission, routes::AppState};

#[derive(Debug, Deserialize)]
pub struct ProductListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub category_id: Option<Uuid>,
    pub brand_id: Option<Uuid>,
    pub status: Option<String>,
    pub low_stock: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CreateProductRequest {
    pub sku: String,
    pub barcode: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub category_id: Uuid,
    pub brand_id: Option<Uuid>,
    pub unit_id: Uuid,
    pub purchase_price: Decimal,
    pub retail_price: Decimal,
    pub wholesale_price: Decimal,
    pub gst_rate: Decimal,
    pub hsn_code: Option<String>,
    pub reorder_threshold: Option<i32>,
    pub reorder_quantity: Option<i32>,
}

pub async fn list_products(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(q): Query<ProductListQuery>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "inventory.read")?;

    let page = q.page.unwrap_or(1).max(1);
    let per_page = q.per_page.unwrap_or(20).min(100);
    let offset = (page - 1) * per_page;

    let rows = sqlx::query!(
        r#"
        SELECT
            p.id, p.sku, p.barcode, p.name, p.description,
            p.purchase_price, p.retail_price, p.wholesale_price,
            p.gst_rate, p.hsn_code, p.status, p.created_at, p.updated_at,
            c.name as "category_name?",
            b.name as "brand_name?",
            u.name as "unit_name?", u.abbreviation as "unit_abbr?",
            COALESCE(SUM(ib.on_hand), 0) as total_stock,
            COALESCE(SUM(ib.on_hand - ib.reserved), 0) as available_stock
        FROM products p
        LEFT JOIN categories c ON c.id = p.category_id
        LEFT JOIN brands b ON b.id = p.brand_id
        LEFT JOIN units_of_measure u ON u.id = p.unit_id
        LEFT JOIN inventory_balances ib ON ib.product_id = p.id
        WHERE ($1::text IS NULL OR p.name ILIKE '%' || $1 || '%' OR p.sku ILIKE '%' || $1 || '%')
          AND ($2::uuid IS NULL OR p.category_id = $2)
          AND ($3::text IS NULL OR p.status = $3)
        GROUP BY p.id, c.name, b.name, u.name, u.abbreviation
        ORDER BY p.created_at DESC
        LIMIT $4 OFFSET $5
        "#,
        q.search,
        q.category_id,
        q.status,
        per_page,
        offset
    )
    .fetch_all(&state.pool)
    .await?;

    let total: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM products")
        .fetch_one(&state.pool).await?.unwrap_or(0);

    let data: Vec<serde_json::Value> = rows.iter().map(|r| serde_json::json!({
        "id": r.id,
        "sku": r.sku,
        "barcode": r.barcode,
        "name": r.name,
        "description": r.description,
        "purchase_price": r.purchase_price,
        "retail_price": r.retail_price,
        "wholesale_price": r.wholesale_price,
        "gst_rate": r.gst_rate,
        "hsn_code": r.hsn_code,
        "status": r.status,
        "category_name": r.category_name,
        "brand_name": r.brand_name,
        "unit_name": r.unit_name,
        "unit_abbr": r.unit_abbr,
        "total_stock": r.total_stock,
        "available_stock": r.available_stock,
        "created_at": r.created_at,
        "updated_at": r.updated_at,
    })).collect();

    Ok(Json(serde_json::json!({
        "data": data,
        "total": total,
        "page": page,
        "per_page": per_page,
        "total_pages": (total as f64 / per_page as f64).ceil() as i64
    })))
}

pub async fn get_product(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "inventory.read")?;

    let row = sqlx::query!(
        r#"
        SELECT p.id, p.sku, p.barcode, p.name, p.description, p.status,
               p.purchase_price, p.retail_price, p.wholesale_price,
               p.gst_rate, p.hsn_code, p.reorder_threshold, p.image_url,
               p.created_at, p.updated_at,
               c.name as "category_name?",
               b.name as "brand_name?",
               u.name as "unit_name?",
               COALESCE(
                   json_agg(json_build_object(
                       'location_id', ib.location_id,
                       'rack_code', rl.rack_code,
                       'warehouse_name', w.name,
                       'on_hand', ib.on_hand,
                       'available', (ib.on_hand - ib.reserved)
                   )) FILTER (WHERE ib.id IS NOT NULL),
                   '[]'::json
               ) as locations
        FROM products p
        LEFT JOIN categories c ON c.id = p.category_id
        LEFT JOIN brands b ON b.id = p.brand_id
        LEFT JOIN units_of_measure u ON u.id = p.unit_id
        LEFT JOIN inventory_balances ib ON ib.product_id = p.id
        LEFT JOIN rack_locations rl ON rl.id = ib.location_id
        LEFT JOIN warehouses w ON w.id = rl.warehouse_id
        WHERE p.id = $1
        GROUP BY p.id, c.name, b.name, u.name
        "#,
        id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Product {} not found", id)))?;

    Ok(Json(serde_json::json!({
        "id": row.id,
        "sku": row.sku,
        "barcode": row.barcode,
        "name": row.name,
        "description": row.description,
        "category_name": row.category_name,
        "brand_name": row.brand_name,
        "unit_name": row.unit_name,
        "purchase_price": row.purchase_price,
        "retail_price": row.retail_price,
        "wholesale_price": row.wholesale_price,
        "gst_rate": row.gst_rate,
        "hsn_code": row.hsn_code,
        "status": row.status,
        "reorder_threshold": row.reorder_threshold,
        "image_url": row.image_url,
        "locations": row.locations,
        "created_at": row.created_at,
        "updated_at": row.updated_at,
    })))
}

pub async fn create_product(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(req): Json<CreateProductRequest>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "inventory.create")?;

    let id = sqlx::query_scalar!(
        r#"
        INSERT INTO products (
            sku, barcode, name, description, category_id, brand_id,
            unit_id, purchase_price, retail_price, wholesale_price,
            gst_rate, hsn_code, reorder_threshold, reorder_quantity
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
        RETURNING id
        "#,
        req.sku, req.barcode, req.name, req.description, req.category_id, req.brand_id,
        req.unit_id, req.purchase_price, req.retail_price, req.wholesale_price,
        req.gst_rate, req.hsn_code, req.reorder_threshold.unwrap_or(10),
        req.reorder_quantity.unwrap_or(50)
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        if e.to_string().contains("unique") || e.to_string().contains("duplicate") {
            AppError::Conflict("SKU or barcode already exists".to_string())
        } else {
            AppError::Database(e)
        }
    })?;

    Ok(Json(serde_json::json!({ "id": id, "message": "Product created successfully" })))
}

pub async fn update_product(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "inventory.update")?;

    sqlx::query!(
        r#"
        UPDATE products SET
            name = COALESCE($1, name),
            description = COALESCE($2, description),
            retail_price = COALESCE(($3::text)::numeric, retail_price),
            wholesale_price = COALESCE(($4::text)::numeric, wholesale_price),
            status = COALESCE($5, status),
            updated_at = NOW()
        WHERE id = $6
        "#,
        body.get("name").and_then(|v| v.as_str()),
        body.get("description").and_then(|v| v.as_str()),
        body.get("retail_price").and_then(|v| v.as_str()),
        body.get("wholesale_price").and_then(|v| v.as_str()),
        body.get("status").and_then(|v| v.as_str()),
        id
    )
    .execute(&state.pool)
    .await?;

    Ok(Json(serde_json::json!({ "message": "Product updated" })))
}

pub async fn list_categories(
    State(state): State<AppState>,
    Extension(_): Extension<AuthenticatedUser>,
) -> AppResult<Json<serde_json::Value>> {
    let rows = sqlx::query!("SELECT id, name, slug, description, parent_id FROM categories ORDER BY name")
        .fetch_all(&state.pool).await?;
    let data: Vec<_> = rows.iter().map(|r| serde_json::json!({"id": r.id, "name": r.name, "slug": r.slug})).collect();
    Ok(Json(serde_json::json!({ "data": data })))
}

pub async fn create_category(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "inventory.create")?;
    let name = body.get("name").and_then(|v| v.as_str()).ok_or_else(|| AppError::Validation("name required".into()))?;
    let slug = name.to_lowercase().replace(' ', "_");
    let id = sqlx::query_scalar!(
        "INSERT INTO categories (name, slug) VALUES ($1, $2) RETURNING id",
        name, slug
    ).fetch_one(&state.pool).await?;
    Ok(Json(serde_json::json!({"id": id})))
}

pub async fn list_brands(
    State(state): State<AppState>,
    Extension(_): Extension<AuthenticatedUser>,
) -> AppResult<Json<serde_json::Value>> {
    let rows = sqlx::query!("SELECT id, name FROM brands ORDER BY name").fetch_all(&state.pool).await?;
    let data: Vec<_> = rows.iter().map(|r| serde_json::json!({"id": r.id, "name": r.name})).collect();
    Ok(Json(serde_json::json!({ "data": data })))
}

pub async fn create_brand(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "inventory.create")?;
    let name = body.get("name").and_then(|v| v.as_str()).ok_or_else(|| AppError::Validation("name required".into()))?;
    let id = sqlx::query_scalar!("INSERT INTO brands (name) VALUES ($1) RETURNING id", name)
        .fetch_one(&state.pool).await?;
    Ok(Json(serde_json::json!({"id": id})))
}

pub async fn list_units(
    State(state): State<AppState>,
    Extension(_): Extension<AuthenticatedUser>,
) -> AppResult<Json<serde_json::Value>> {
    let rows = sqlx::query!("SELECT id, name, abbreviation FROM units_of_measure ORDER BY name")
        .fetch_all(&state.pool).await?;
    let data: Vec<_> = rows.iter().map(|r| serde_json::json!({"id": r.id, "name": r.name, "abbreviation": r.abbreviation})).collect();
    Ok(Json(serde_json::json!({ "data": data })))
}

pub async fn list_imports(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(q): Query<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "inventory.import")?;
    let rows = sqlx::query!(
        r#"SELECT id, filename, status, mode, total_rows, success_count, error_count, created_at
           FROM inventory_imports ORDER BY created_at DESC LIMIT 50"#
    ).fetch_all(&state.pool).await?;
    let data: Vec<_> = rows.iter().map(|r| serde_json::json!({
        "id": r.id, "filename": r.filename, "status": r.status,
        "mode": r.mode, "total_rows": r.total_rows,
        "success_count": r.success_count, "error_count": r.error_count,
        "created_at": r.created_at
    })).collect();
    Ok(Json(serde_json::json!({"data": data})))
}

pub async fn upload_import(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    mut multipart: Multipart,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "inventory.import")?;

    let mut filename = String::new();
    let mut file_bytes: Vec<u8> = Vec::new();
    let mut mode = "create".to_string();

    while let Some(field) = multipart.next_field().await.map_err(|e| AppError::Validation(e.to_string()))? {
        match field.name() {
            Some("file") => {
                filename = field.file_name().unwrap_or("upload.csv").to_string();
                file_bytes = field.bytes().await.map_err(|e| AppError::Validation(e.to_string()))?.to_vec();
            }
            Some("mode") => {
                mode = field.text().await.unwrap_or_else(|_| "create".to_string());
            }
            _ => {}
        }
    }

    if file_bytes.is_empty() {
        return Err(AppError::Validation("No file provided".to_string()));
    }

    // Create import record
    let import_id = sqlx::query_scalar!(
        r#"INSERT INTO inventory_imports (filename, mode, status, created_by)
           VALUES ($1, $2, 'pending', $3) RETURNING id"#,
        filename, mode, user.id
    ).fetch_one(&state.pool).await?;

    // Process in background (spawn task)
    let pool = state.pool.clone();
    let user_id = user.id;
    let file_data = file_bytes.clone();
    tokio::spawn(async move {
        crate::inventory::service::process_import(pool, import_id, file_data, user_id, &mode).await.ok();
    });

    Ok(Json(serde_json::json!({
        "id": import_id,
        "message": "Import started. Check import status for progress.",
        "filename": filename
    })))
}

pub async fn get_import(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "inventory.import")?;
    let row = sqlx::query!(
        "SELECT * FROM inventory_imports WHERE id = $1", id
    ).fetch_optional(&state.pool).await?
     .ok_or_else(|| AppError::NotFound("Import not found".into()))?;
    Ok(Json(serde_json::json!({
        "id": row.id, "filename": row.filename, "status": row.status,
        "mode": row.mode, "total_rows": row.total_rows,
        "success_count": row.success_count, "error_count": row.error_count,
        "created_at": row.created_at, "completed_at": row.completed_at
    })))
}
