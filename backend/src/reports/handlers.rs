use axum::{extract::{State, Extension}, Json};
use crate::{auth::models::AuthenticatedUser, errors::AppResult, middleware::permission::require_permission, routes::AppState};

pub async fn inventory_report(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "reports.export")?;
    let rows = sqlx::query!(
        r#"SELECT p.id, p.sku, p.name, c.name as "category?", b.name as "brand?",
                  COALESCE(SUM(ib.on_hand), 0) as total_stock,
                  COALESCE(SUM(ib.available), 0) as available,
                  p.retail_price,
                  COALESCE(SUM(ib.on_hand), 0) * p.purchase_price as inventory_value
           FROM products p
           LEFT JOIN categories c ON c.id = p.category_id
           LEFT JOIN brands b ON b.id = p.brand_id
           LEFT JOIN inventory_balances ib ON ib.product_id = p.id
           WHERE p.status = 'active'
           GROUP BY p.id, c.name, b.name
           ORDER BY p.name"#
    ).fetch_all(&state.pool).await?;

    let data: Vec<_> = rows.iter().map(|r| serde_json::json!({
        "sku": r.sku, "name": r.name, "category": r.category, "brand": r.brand,
        "total_stock": r.total_stock, "available": r.available,
        "retail_price": r.retail_price, "inventory_value": r.inventory_value
    })).collect();
    Ok(Json(serde_json::json!({"data": data, "total": data.len()})))
}

pub async fn sales_report(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "reports.export")?;
    let rows = sqlx::query!(
        r#"SELECT si.invoice_number, si.invoice_date, si.grand_total, si.amount_paid,
                  si.outstanding_amount, si.status, c.name as "customer_name?"
           FROM sales_invoices si LEFT JOIN customers c ON c.id = si.customer_id
           ORDER BY si.invoice_date DESC LIMIT 500"#
    ).fetch_all(&state.pool).await?;

    let data: Vec<_> = rows.iter().map(|r| serde_json::json!({
        "invoice_number": r.invoice_number, "date": r.invoice_date,
        "customer": r.customer_name, "total": r.grand_total,
        "paid": r.amount_paid, "outstanding": r.outstanding_amount, "status": r.status
    })).collect();
    Ok(Json(serde_json::json!({"data": data})))
}

pub async fn purchases_report(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "reports.export")?;
    let rows = sqlx::query!(
        r#"SELECT po.po_number, po.purchase_date, po.grand_total, po.advance_paid,
                  po.outstanding_amount, po.status, po.payment_status, s.business_name
           FROM purchase_orders po JOIN suppliers s ON s.id = po.supplier_id
           ORDER BY po.purchase_date DESC LIMIT 500"#
    ).fetch_all(&state.pool).await?;

    let data: Vec<_> = rows.iter().map(|r| serde_json::json!({
        "po_number": r.po_number, "date": r.purchase_date,
        "supplier": r.business_name, "total": r.grand_total,
        "advance": r.advance_paid, "outstanding": r.outstanding_amount,
        "status": r.status, "payment_status": r.payment_status
    })).collect();
    Ok(Json(serde_json::json!({"data": data})))
}

pub async fn payables_report(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "reports.export")?;
    crate::finance::handlers::list_payables(State(state), Extension(user)).await
}

pub async fn receivables_report(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "reports.export")?;
    crate::finance::handlers::list_receivables(State(state), Extension(user)).await
}
