use axum::{
    extract::{Extension, Query, State},
    Json,
};
use serde_json::{json, Value};

use crate::{
    auth::models::AuthenticatedUser,
    common::{non_empty, paginated, uuid, ListQuery},
    errors::{AppError, AppResult},
    middleware::permission::require_permission,
    routes::AppState,
};

pub async fn list_movements(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "inventory.read")?;
    let kind = non_empty(&q.kind);
    let search = q.search();

    let rows = sqlx::query!(
        r#"SELECT sm.id, sm.product_id, sm.quantity, sm.movement_type, sm.reference_document,
                  sm.timestamp, sm.reason, sm.notes, sm.source_location_id, sm.destination_location_id,
                  p.name AS product_name, p.sku,
                  src.rack_code AS "source_rack?", dst.rack_code AS "destination_rack?",
                  u.full_name AS "user_name?",
                  COALESCE(si.invoice_number, po.po_number) AS "document_number?",
                  COUNT(*) OVER () AS "full_count!"
           FROM stock_movements sm
           JOIN products p ON p.id = sm.product_id
           LEFT JOIN rack_locations src ON src.id = sm.source_location_id
           LEFT JOIN rack_locations dst ON dst.id = sm.destination_location_id
           LEFT JOIN users u ON u.id = sm.user_id
           LEFT JOIN sales_invoices si ON si.id::text = sm.reference_document
           LEFT JOIN purchase_orders po ON po.id::text = sm.reference_document
           WHERE ($1::uuid IS NULL OR sm.product_id = $1)
             AND ($2::text IS NULL OR sm.movement_type = $2)
             AND ($3::text IS NULL OR p.name ILIKE '%' || $3 || '%' OR p.sku ILIKE '%' || $3 || '%')
             AND ($4::date IS NULL OR sm.timestamp >= $4::date)
             AND ($5::date IS NULL OR sm.timestamp < $5::date + 1)
           ORDER BY sm.timestamp DESC
           LIMIT $6 OFFSET $7"#,
        q.product_id, kind, search, q.from, q.to, q.per_page(), q.offset()
    )
    .fetch_all(&state.pool)
    .await?;

    let total = rows.first().map(|r| r.full_count).unwrap_or(0);
    let data = rows
        .into_iter()
        .map(|r| {
            json!({
                "id": r.id, "product_id": r.product_id, "product_name": r.product_name, "sku": r.sku,
                "quantity": r.quantity, "type": r.movement_type, "movement_type": r.movement_type,
                "reference": r.document_number.clone().or(r.reference_document.clone()),
                "reference_document": r.document_number.or(r.reference_document),
                "source_location_id": r.source_location_id, "destination_location_id": r.destination_location_id,
                "source_rack": r.source_rack, "destination_rack": r.destination_rack,
                "user_name": r.user_name, "timestamp": r.timestamp, "reason": r.reason, "notes": r.notes,
            })
        })
        .collect();

    Ok(Json(paginated(data, total, &q)))
}

pub async fn create_stock_count(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(body): Json<Value>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "inventory.count")?;
    let location_id = uuid(body.get("location_id")).ok_or_else(|| AppError::Validation("location_id is required".into()))?;

    let mut tx = state.pool.begin().await?;
    let id = sqlx::query_scalar!(
        "INSERT INTO stock_counts (location_id, created_by) VALUES ($1, $2) RETURNING id",
        location_id, user.id
    )
    .fetch_one(&mut *tx)
    .await?;

    // Snapshot current system quantities so the counter can fill in actual counts.
    sqlx::query!(
        r#"INSERT INTO stock_count_items (count_id, product_id, system_quantity)
           SELECT $1, product_id, on_hand FROM inventory_balances WHERE location_id = $2"#,
        id, location_id
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(Json(json!({"id": id, "message": "Stock count started"})))
}
