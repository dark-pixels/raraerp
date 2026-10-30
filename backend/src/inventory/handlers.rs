use axum::{
    extract::{Extension, Multipart, Path, Query, State},
    Json,
};
use rust_decimal::Decimal;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    audit::service::log_action,
    auth::models::AuthenticatedUser,
    common::{dec, dec_field, int, paginated, required_text, text, uuid, ListQuery},
    errors::{AppError, AppResult},
    inventory::stock,
    middleware::permission::require_permission,
    routes::AppState,
};

const PRODUCT_STATUSES: &[&str] = &["active", "inactive", "discontinued"];

pub async fn list_products(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "inventory.read")?;
    let search = q.search();
    let status = q.status();
    let low_stock = q.low_stock.unwrap_or(false);

    let rows = sqlx::query!(
        r#"
        WITH stock AS (
            SELECT product_id, SUM(on_hand) AS on_hand, SUM(available) AS available
            FROM inventory_balances GROUP BY product_id
        )
        SELECT
            p.id, p.sku, p.barcode, p.name, p.description, p.image_url,
            p.category_id, p.brand_id, p.unit_id, p.manufacturer,
            p.purchase_price, p.retail_price, p.wholesale_price, p.min_selling_price,
            p.gst_rate, p.hsn_code, p.reorder_threshold, p.reorder_quantity,
            p.status, p.created_at, p.updated_at,
            c.name AS "category_name?", b.name AS "brand_name?",
            u.name AS "unit_name?", u.abbreviation AS "unit_abbr?",
            COALESCE(s.on_hand, 0) AS "total_stock!",
            COALESCE(s.available, 0) AS "available_stock!",
            COUNT(*) OVER () AS "full_count!"
        FROM products p
        LEFT JOIN categories c ON c.id = p.category_id
        LEFT JOIN brands b ON b.id = p.brand_id
        LEFT JOIN units_of_measure u ON u.id = p.unit_id
        LEFT JOIN stock s ON s.product_id = p.id
        WHERE ($1::text IS NULL OR p.name ILIKE '%' || $1 || '%' OR p.sku ILIKE '%' || $1 || '%'
               OR p.barcode = $1 OR p.hsn_code ILIKE $1 || '%')
          AND ($2::uuid IS NULL OR p.category_id = $2)
          AND ($3::uuid IS NULL OR p.brand_id = $3)
          AND ($4::text IS NULL OR p.status = $4)
          AND (NOT $5 OR COALESCE(s.available, 0) < p.reorder_threshold)
        ORDER BY (p.barcode = $1 OR p.sku ILIKE $1) DESC NULLS LAST, p.name
        LIMIT $6 OFFSET $7
        "#,
        search, q.category_id, q.brand_id, status, low_stock, q.per_page(), q.offset()
    )
    .fetch_all(&state.pool)
    .await?;

    let total = rows.first().map(|r| r.full_count).unwrap_or(0);
    let data = rows
        .into_iter()
        .map(|r| {
            json!({
                "id": r.id, "sku": r.sku, "barcode": r.barcode, "name": r.name,
                "description": r.description, "image_url": r.image_url,
                "category_id": r.category_id, "brand_id": r.brand_id, "unit_id": r.unit_id,
                "manufacturer": r.manufacturer,
                "purchase_price": r.purchase_price, "retail_price": r.retail_price,
                "wholesale_price": r.wholesale_price, "min_selling_price": r.min_selling_price,
                "gst_rate": r.gst_rate, "hsn_code": r.hsn_code,
                "reorder_threshold": r.reorder_threshold, "reorder_quantity": r.reorder_quantity,
                "status": r.status,
                "category_name": r.category_name, "brand_name": r.brand_name,
                "unit_name": r.unit_name, "unit_abbr": r.unit_abbr,
                "total_stock": r.total_stock, "available_stock": r.available_stock,
                "is_low_stock": r.available_stock < Decimal::from(r.reorder_threshold),
                "created_at": r.created_at, "updated_at": r.updated_at,
            })
        })
        .collect();

    // A page past the end returns no rows (and so no window count); report the real total.
    let total = if total == 0 && q.page() > 1 {
        sqlx::query_scalar!(r#"SELECT COUNT(*) AS "c!" FROM products"#).fetch_one(&state.pool).await?
    } else {
        total
    };

    Ok(Json(paginated(data, total, &q)))
}

pub async fn get_product(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "inventory.read")?;

    let row = sqlx::query!(
        r#"
        SELECT p.*,
               c.name AS "category_name?", b.name AS "brand_name?", u.name AS "unit_name?",
               COALESCE((
                   SELECT json_agg(json_build_object(
                       'location_id', ib.location_id, 'rack_code', rl.rack_code,
                       'shelf', rl.shelf, 'bin', rl.bin, 'warehouse_name', w.name,
                       'on_hand', ib.on_hand, 'available', ib.available) ORDER BY w.name, rl.rack_code)
                   FROM inventory_balances ib
                   JOIN rack_locations rl ON rl.id = ib.location_id
                   JOIN warehouses w ON w.id = rl.warehouse_id
                   WHERE ib.product_id = p.id AND ib.on_hand > 0
               ), '[]'::json) AS "locations!"
        FROM products p
        LEFT JOIN categories c ON c.id = p.category_id
        LEFT JOIN brands b ON b.id = p.brand_id
        LEFT JOIN units_of_measure u ON u.id = p.unit_id
        WHERE p.id = $1
        "#,
        id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Product not found".into()))?;

    let total_stock: Decimal = sqlx::query_scalar!(
        r#"SELECT COALESCE(SUM(on_hand), 0) AS "s!" FROM inventory_balances WHERE product_id = $1"#,
        id
    )
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(json!({
        "id": row.id, "sku": row.sku, "barcode": row.barcode, "name": row.name,
        "description": row.description, "manufacturer": row.manufacturer,
        "category_id": row.category_id, "category_name": row.category_name,
        "brand_id": row.brand_id, "brand_name": row.brand_name,
        "unit_id": row.unit_id, "unit_name": row.unit_name,
        "purchase_price": row.purchase_price, "retail_price": row.retail_price,
        "wholesale_price": row.wholesale_price, "min_selling_price": row.min_selling_price,
        "gst_rate": row.gst_rate, "hsn_code": row.hsn_code,
        "reorder_threshold": row.reorder_threshold, "reorder_quantity": row.reorder_quantity,
        "status": row.status, "image_url": row.image_url,
        "total_stock": total_stock, "locations": row.locations,
        "created_at": row.created_at, "updated_at": row.updated_at,
    })))
}

fn validate_prices(body: &Value) -> AppResult<()> {
    for field in ["purchase_price", "retail_price", "wholesale_price", "min_selling_price"] {
        if let Some(v) = dec_field(body, field)? {
            if v < Decimal::ZERO {
                return Err(AppError::Validation(format!("{} cannot be negative", field.replace('_', " "))));
            }
        }
    }
    if let Some(g) = dec_field(body, "gst_rate")? {
        if g < Decimal::ZERO || g > Decimal::from(100) {
            return Err(AppError::Validation("GST rate must be between 0 and 100".into()));
        }
    }
    Ok(())
}

async fn default_unit(state: &AppState) -> AppResult<Uuid> {
    sqlx::query_scalar!("SELECT id FROM units_of_measure ORDER BY (abbreviation = 'Pcs') DESC, name LIMIT 1")
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::Validation("Create a unit of measure first".into()))
}

pub async fn create_product(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(body): Json<Value>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "inventory.create")?;

    let sku = required_text(&body, "sku")?.to_uppercase();
    let name = required_text(&body, "name")?;
    let category_id = uuid(body.get("category_id")).ok_or_else(|| AppError::Validation("Select a category".into()))?;
    let unit_id = match uuid(body.get("unit_id")) {
        Some(u) => u,
        None => default_unit(&state).await?,
    };
    validate_prices(&body)?;
    let opening_stock = dec_field(&body, "opening_stock")?.unwrap_or(Decimal::ZERO);
    if opening_stock < Decimal::ZERO {
        return Err(AppError::Validation("Opening stock cannot be negative".into()));
    }

    let mut tx = state.pool.begin().await?;
    let id = sqlx::query_scalar!(
        r#"INSERT INTO products (
            sku, barcode, name, description, category_id, brand_id, manufacturer,
            unit_id, purchase_price, retail_price, wholesale_price, min_selling_price,
            gst_rate, hsn_code, reorder_threshold, reorder_quantity, status, created_by
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18)
        RETURNING id"#,
        sku, text(body.get("barcode")), name, text(body.get("description")),
        category_id, uuid(body.get("brand_id")), text(body.get("manufacturer")), unit_id,
        dec(body.get("purchase_price")).unwrap_or(Decimal::ZERO),
        dec(body.get("retail_price")).unwrap_or(Decimal::ZERO),
        dec(body.get("wholesale_price")).unwrap_or(Decimal::ZERO),
        dec(body.get("min_selling_price")),
        dec(body.get("gst_rate")).unwrap_or(Decimal::from(18)),
        text(body.get("hsn_code")),
        int(body.get("reorder_threshold")).unwrap_or(10) as i32,
        int(body.get("reorder_quantity")).unwrap_or(50) as i32,
        "active", user.id
    )
    .fetch_one(&mut *tx)
    .await?;

    if opening_stock > Decimal::ZERO {
        let location = stock::resolve_location(&mut tx, uuid(body.get("location_id"))).await?;
        stock::add_stock(&mut tx, id, location, opening_stock, "opening_stock", None, user.id, Some("Opening stock")).await?;
    }
    tx.commit().await?;

    log_action(&state.pool, user.id, "product_created", "product", Some(&id.to_string()), Some(json!({"sku": sku, "name": name}))).await;
    Ok(Json(json!({ "id": id, "message": "Product created successfully" })))
}

pub async fn update_product(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
    Json(body): Json<Value>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "inventory.update")?;
    validate_prices(&body)?;

    let status = text(body.get("status"));
    if let Some(s) = &status {
        if !PRODUCT_STATUSES.contains(&s.as_str()) {
            return Err(AppError::Validation("Status must be active, inactive or discontinued".into()));
        }
    }
    // An explicit empty barcode clears it; an absent field leaves it unchanged.
    let clear_barcode = body.get("barcode").map(|v| v.as_str().map(str::trim) == Some("")).unwrap_or(false);

    let res = sqlx::query!(
        r#"UPDATE products SET
            sku = COALESCE(UPPER($1), sku),
            name = COALESCE($2, name),
            description = COALESCE($3, description),
            barcode = CASE WHEN $4 THEN NULL ELSE COALESCE($5, barcode) END,
            category_id = COALESCE($6, category_id),
            brand_id = COALESCE($7, brand_id),
            unit_id = COALESCE($8, unit_id),
            purchase_price = COALESCE($9, purchase_price),
            retail_price = COALESCE($10, retail_price),
            wholesale_price = COALESCE($11, wholesale_price),
            min_selling_price = COALESCE($12, min_selling_price),
            gst_rate = COALESCE($13, gst_rate),
            hsn_code = COALESCE($14, hsn_code),
            reorder_threshold = COALESCE($15, reorder_threshold),
            reorder_quantity = COALESCE($16, reorder_quantity),
            status = COALESCE($17, status),
            image_url = COALESCE($18, image_url),
            manufacturer = COALESCE($19, manufacturer),
            updated_at = NOW()
        WHERE id = $20"#,
        text(body.get("sku")), text(body.get("name")), text(body.get("description")),
        clear_barcode, text(body.get("barcode")),
        uuid(body.get("category_id")), uuid(body.get("brand_id")), uuid(body.get("unit_id")),
        dec(body.get("purchase_price")), dec(body.get("retail_price")), dec(body.get("wholesale_price")),
        dec(body.get("min_selling_price")), dec(body.get("gst_rate")), text(body.get("hsn_code")),
        int(body.get("reorder_threshold")).map(|v| v as i32), int(body.get("reorder_quantity")).map(|v| v as i32),
        status, text(body.get("image_url")), text(body.get("manufacturer")),
        id
    )
    .execute(&state.pool)
    .await?;

    if res.rows_affected() == 0 {
        return Err(AppError::NotFound("Product not found".into()));
    }
    log_action(&state.pool, user.id, "product_updated", "product", Some(&id.to_string()), Some(body)).await;
    Ok(Json(json!({ "message": "Product updated" })))
}

/// Manual stock correction (damage, loss, found stock, recount).
pub async fn adjust_stock(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(body): Json<Value>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "inventory.adjust")?;
    let product_id = uuid(body.get("product_id")).ok_or_else(|| AppError::Validation("product_id is required".into()))?;
    let reason = required_text(&body, "reason")?;
    let delta = dec_field(&body, "quantity")?;
    let new_quantity = dec_field(&body, "new_quantity")?;

    let mut tx = state.pool.begin().await?;
    let location = stock::resolve_location(&mut tx, uuid(body.get("location_id"))).await?;
    let current: Decimal = sqlx::query_scalar!(
        "SELECT on_hand FROM inventory_balances WHERE product_id = $1 AND location_id = $2 FOR UPDATE",
        product_id, location
    )
    .fetch_optional(&mut *tx)
    .await?
    .unwrap_or(Decimal::ZERO);

    let change = match (delta, new_quantity) {
        (_, Some(target)) if target >= Decimal::ZERO => target - current,
        (_, Some(_)) => return Err(AppError::Validation("New quantity cannot be negative".into())),
        (Some(d), None) => d,
        (None, None) => return Err(AppError::Validation("Provide quantity (+/-) or new_quantity".into())),
    };
    if change == Decimal::ZERO {
        return Err(AppError::Validation("Stock is already at that quantity".into()));
    }

    let movement = match text(body.get("type")).as_deref() {
        Some("damaged") => "damaged_stock",
        Some("received") if change > Decimal::ZERO => "purchase_receipt",
        _ => "approved_adjustment",
    };
    if change > Decimal::ZERO {
        stock::add_stock(&mut tx, product_id, location, change, movement, None, user.id, Some(&reason)).await?;
    } else {
        stock::remove_stock_at(&mut tx, product_id, location, -change, movement, None, user.id, Some(&reason)).await?;
    }
    tx.commit().await?;

    log_action(&state.pool, user.id, "stock_adjusted", "product", Some(&product_id.to_string()),
        Some(json!({"change": change, "reason": reason, "location_id": location}))).await;

    Ok(Json(json!({"message": "Stock adjusted", "change": change, "new_quantity": current + change})))
}

fn slugify(name: &str) -> String {
    let mut slug = String::new();
    for c in name.trim().to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.trim_matches('-').to_string()
}

pub async fn list_categories(
    State(state): State<AppState>,
    Extension(_): Extension<AuthenticatedUser>,
) -> AppResult<Json<Value>> {
    let rows = sqlx::query!(
        r#"SELECT c.id, c.name, c.slug, c.description, c.parent_id,
                  (SELECT COUNT(*) FROM products p WHERE p.category_id = c.id) AS "product_count!"
           FROM categories c ORDER BY c.name"#
    )
    .fetch_all(&state.pool)
    .await?;
    let data: Vec<Value> = rows
        .iter()
        .map(|r| json!({"id": r.id, "name": r.name, "slug": r.slug, "description": r.description, "parent_id": r.parent_id, "product_count": r.product_count}))
        .collect();
    Ok(Json(json!({ "data": data })))
}

pub async fn create_category(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(body): Json<Value>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "inventory.create")?;
    let name = required_text(&body, "name")?;
    let slug = slugify(&name);
    if slug.is_empty() {
        return Err(AppError::Validation("Category name must contain letters or numbers".into()));
    }
    let id = sqlx::query_scalar!(
        "INSERT INTO categories (name, slug, description, parent_id) VALUES ($1, $2, $3, $4) RETURNING id",
        name, slug, text(body.get("description")), uuid(body.get("parent_id"))
    )
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(json!({"id": id, "name": name})))
}

pub async fn list_brands(
    State(state): State<AppState>,
    Extension(_): Extension<AuthenticatedUser>,
) -> AppResult<Json<Value>> {
    let rows = sqlx::query!("SELECT id, name, logo_url FROM brands ORDER BY name").fetch_all(&state.pool).await?;
    let data: Vec<Value> = rows.iter().map(|r| json!({"id": r.id, "name": r.name, "logo_url": r.logo_url})).collect();
    Ok(Json(json!({ "data": data })))
}

pub async fn create_brand(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(body): Json<Value>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "inventory.create")?;
    let name = required_text(&body, "name")?;
    let id = sqlx::query_scalar!("INSERT INTO brands (name) VALUES ($1) RETURNING id", name)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(json!({"id": id, "name": name})))
}

pub async fn list_units(
    State(state): State<AppState>,
    Extension(_): Extension<AuthenticatedUser>,
) -> AppResult<Json<Value>> {
    let rows = sqlx::query!("SELECT id, name, abbreviation FROM units_of_measure ORDER BY name")
        .fetch_all(&state.pool)
        .await?;
    let data: Vec<Value> = rows.iter().map(|r| json!({"id": r.id, "name": r.name, "abbreviation": r.abbreviation})).collect();
    Ok(Json(json!({ "data": data })))
}

pub async fn create_unit(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(body): Json<Value>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "inventory.create")?;
    let name = required_text(&body, "name")?;
    let abbreviation = required_text(&body, "abbreviation")?;
    let id = sqlx::query_scalar!(
        "INSERT INTO units_of_measure (name, abbreviation) VALUES ($1, $2) RETURNING id",
        name, abbreviation
    )
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(json!({"id": id})))
}

pub async fn list_imports(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "inventory.import")?;
    let rows = sqlx::query!(
        r#"SELECT i.id, i.filename, i.status, i.mode, i.total_rows, i.success_count, i.error_count,
                  i.created_at, i.completed_at, u.full_name AS "created_by_name?"
           FROM inventory_imports i LEFT JOIN users u ON u.id = i.created_by
           ORDER BY i.created_at DESC LIMIT $1"#,
        q.per_page().min(100)
    )
    .fetch_all(&state.pool)
    .await?;
    let data: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.id, "filename": r.filename, "status": r.status,
                "mode": r.mode, "total_rows": r.total_rows,
                "success_count": r.success_count, "error_count": r.error_count,
                "created_at": r.created_at, "completed_at": r.completed_at,
                "created_by_name": r.created_by_name
            })
        })
        .collect();
    Ok(Json(json!({"data": data})))
}

const MAX_IMPORT_BYTES: usize = 10 * 1024 * 1024;

pub async fn upload_import(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    mut multipart: Multipart,
) -> AppResult<Json<Value>> {
    require_permission(&user, "inventory.import")?;

    let mut filename = String::new();
    let mut file_bytes: Vec<u8> = Vec::new();
    let mut mode = "create".to_string();
    let mut location_id: Option<Uuid> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| AppError::Validation(e.to_string()))? {
        match field.name() {
            Some("file") => {
                filename = field.file_name().unwrap_or("upload.csv").to_string();
                file_bytes = field.bytes().await.map_err(|e| AppError::Validation(e.to_string()))?.to_vec();
            }
            Some("mode") => mode = field.text().await.unwrap_or_default().trim().to_string(),
            Some("location_id") => location_id = field.text().await.ok().and_then(|s| s.trim().parse().ok()),
            _ => {}
        }
    }

    if file_bytes.is_empty() {
        return Err(AppError::Validation("Choose a CSV or Excel file to import".into()));
    }
    if file_bytes.len() > MAX_IMPORT_BYTES {
        return Err(AppError::Validation("File is too large (max 10 MB)".into()));
    }
    if !["create", "update", "opening_stock", "adjustment"].contains(&mode.as_str()) {
        return Err(AppError::Validation("Invalid import mode".into()));
    }

    let import_id = sqlx::query_scalar!(
        "INSERT INTO inventory_imports (filename, mode, status, created_by) VALUES ($1, $2, 'processing', $3) RETURNING id",
        filename, mode, user.id
    )
    .fetch_one(&state.pool)
    .await?;

    // Processed inline: background tasks are not reliable on serverless hosts.
    let summary = crate::inventory::service::process_import(&state.pool, import_id, &filename, &file_bytes, user.id, &mode, location_id).await;

    match summary {
        Ok(s) => {
            log_action(&state.pool, user.id, "inventory_imported", "inventory_import", Some(&import_id.to_string()),
                Some(json!({"filename": filename, "mode": mode, "success": s.success, "errors": s.errors}))).await;
            Ok(Json(json!({
                "id": import_id,
                "filename": filename,
                "total_rows": s.total,
                "success_count": s.success,
                "error_count": s.errors,
                "errors": s.messages,
                "message": format!("Imported {} of {} rows", s.success, s.total)
            })))
        }
        Err(e) => {
            sqlx::query!("UPDATE inventory_imports SET status = 'failed', completed_at = NOW() WHERE id = $1", import_id)
                .execute(&state.pool)
                .await
                .ok();
            Err(e)
        }
    }
}

pub async fn get_import(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "inventory.import")?;
    let row = sqlx::query!("SELECT * FROM inventory_imports WHERE id = $1", id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Import not found".into()))?;
    let errors = sqlx::query!(
        "SELECT row_number, error_message, raw_data FROM inventory_import_errors WHERE import_id = $1 ORDER BY row_number LIMIT 500",
        id
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(json!({
        "id": row.id, "filename": row.filename, "status": row.status,
        "mode": row.mode, "total_rows": row.total_rows,
        "success_count": row.success_count, "error_count": row.error_count,
        "created_at": row.created_at, "completed_at": row.completed_at,
        "errors": errors.iter().map(|e| json!({"row": e.row_number, "message": e.error_message, "data": e.raw_data})).collect::<Vec<_>>()
    })))
}

#[cfg(test)]
mod tests {
    use super::slugify;

    #[test]
    fn slugs() {
        assert_eq!(slugify("Screws & Fasteners"), "screws-fasteners");
        assert_eq!(slugify("  Hinges "), "hinges");
        assert_eq!(slugify("Soft-Close  Drawer"), "soft-close-drawer");
    }
}
