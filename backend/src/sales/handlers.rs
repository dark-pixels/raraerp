use axum::{
    extract::{Extension, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{auth::models::AuthenticatedUser, common::today, errors::AppResult, routes::AppState};

pub async fn dashboard_summary(
    State(state): State<AppState>,
    Extension(_user): Extension<AuthenticatedUser>,
) -> AppResult<Json<Value>> {
    let today = today();
    let row = sqlx::query!(
        r#"SELECT
             (SELECT COUNT(*) FROM products WHERE status = 'active') AS "total_products!",
             (SELECT COALESCE(SUM(grand_total), 0) FROM sales_invoices WHERE invoice_date = $1 AND status <> 'cancelled') AS "today_sales!",
             (SELECT COUNT(*) FROM sales_invoices WHERE invoice_date = $1 AND status <> 'cancelled') AS "today_invoices!",
             (SELECT COALESCE(SUM(grand_total), 0) FROM sales_invoices
                WHERE date_trunc('month', invoice_date) = date_trunc('month', $1::date) AND status <> 'cancelled') AS "monthly_sales!",
             (SELECT COALESCE(SUM(grand_total), 0) FROM sales_invoices
                WHERE date_trunc('month', invoice_date) = date_trunc('month', $1::date - INTERVAL '1 month') AND status <> 'cancelled') AS "last_month_sales!",
             (SELECT COALESCE(SUM(outstanding_amount), 0) FROM sales_invoices WHERE outstanding_amount > 0 AND status <> 'cancelled') AS "receivables!",
             (SELECT COALESCE(SUM(outstanding_amount), 0) FROM purchase_orders WHERE outstanding_amount > 0 AND status <> 'cancelled') AS "payables!",
             (SELECT COUNT(*) FROM (
                 SELECT p.id FROM products p
                 LEFT JOIN inventory_balances ib ON ib.product_id = p.id
                 WHERE p.status = 'active'
                 GROUP BY p.id, p.reorder_threshold
                 HAVING COALESCE(SUM(ib.available), 0) < p.reorder_threshold) low) AS "low_stock!",
             (SELECT COUNT(*) FROM sales_invoices WHERE due_date < $1 AND outstanding_amount > 0 AND status <> 'cancelled') AS "overdue_invoices!",
             (SELECT COUNT(*) FROM purchase_orders WHERE due_date < $1 AND outstanding_amount > 0 AND status <> 'cancelled') AS "overdue_purchases!",
             (SELECT COALESCE(SUM(amount), 0) FROM payments WHERE payment_date = $1 AND direction = 'inbound' AND status = 'verified') AS "today_collected!",
             (SELECT COALESCE(SUM(ib.on_hand * p.purchase_price), 0) FROM inventory_balances ib JOIN products p ON p.id = ib.product_id) AS "stock_value!",
             (SELECT COUNT(*) FROM purchase_orders WHERE status IN ('approved','ordered','partially_received')) AS "pending_receipts!",
             (SELECT COUNT(*) FROM expenses WHERE approval_status = 'pending') AS "pending_expenses!""#,
        today
    )
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(json!({
        "total_products": row.total_products,
        "today_sales": row.today_sales,
        "today_invoices": row.today_invoices,
        "monthly_sales": row.monthly_sales,
        "last_month_sales": row.last_month_sales,
        "customer_receivables": row.receivables,
        "supplier_payables": row.payables,
        "low_stock_count": row.low_stock,
        "overdue_customer_invoices": row.overdue_invoices,
        "overdue_supplier_payments": row.overdue_purchases,
        "today_collected": row.today_collected,
        "stock_value": row.stock_value,
        "pending_receipts": row.pending_receipts,
        "pending_expenses": row.pending_expenses,
    })))
}

#[derive(Debug, Deserialize)]
pub struct TrendQuery {
    pub days: Option<String>,
}

/// Daily sales for the last N days (default 30), with zero-filled gaps so charts plot evenly.
pub async fn sales_trend(
    State(state): State<AppState>,
    Extension(_): Extension<AuthenticatedUser>,
    Query(q): Query<TrendQuery>,
) -> AppResult<Json<Value>> {
    let days: i32 = q.days.and_then(|d| d.parse().ok()).unwrap_or(30).clamp(7, 365);
    let rows = sqlx::query!(
        r#"SELECT d::date AS "date!",
                  COALESCE(SUM(si.grand_total), 0) AS "total!",
                  COUNT(si.id) AS "invoices!"
           FROM generate_series($1::date - ($2::int - 1), $1::date, INTERVAL '1 day') d
           LEFT JOIN sales_invoices si ON si.invoice_date = d::date AND si.status <> 'cancelled'
           GROUP BY d ORDER BY d"#,
        today(), days
    )
    .fetch_all(&state.pool)
    .await?;

    let data: Vec<Value> = rows.iter().map(|r| json!({"date": r.date, "total": r.total, "invoices": r.invoices})).collect();
    Ok(Json(json!({"data": data})))
}

pub async fn activity_feed(
    State(state): State<AppState>,
    Extension(_): Extension<AuthenticatedUser>,
) -> AppResult<Json<Value>> {
    let rows = sqlx::query!(
        r#"SELECT * FROM (
               SELECT 'invoice' AS "type!", si.id AS "id!", si.invoice_number AS "reference!",
                      si.grand_total AS "amount!", si.status AS "status!", si.created_at AS "timestamp!",
                      c.name AS "party?"
               FROM sales_invoices si LEFT JOIN customers c ON c.id = si.customer_id
               UNION ALL
               SELECT 'purchase', po.id, po.po_number, po.grand_total, po.status, po.created_at, s.business_name
               FROM purchase_orders po JOIN suppliers s ON s.id = po.supplier_id
               UNION ALL
               SELECT 'payment_' || p.direction, p.id, COALESCE(si.invoice_number, po.po_number, p.payment_method),
                      p.amount, p.status, p.created_at, COALESCE(c.name, s.business_name)
               FROM payments p
               LEFT JOIN sales_invoices si ON p.reference_type = 'invoice' AND si.id = p.reference_id
               LEFT JOIN purchase_orders po ON p.reference_type = 'purchase' AND po.id = p.reference_id
               LEFT JOIN customers c ON c.id = p.customer_id
               LEFT JOIN suppliers s ON s.id = p.supplier_id
           ) a ORDER BY 6 DESC LIMIT 20"#
    )
    .fetch_all(&state.pool)
    .await?;

    let data: Vec<Value> = rows
        .into_iter()
        .map(|r| json!({
            "type": r.r#type, "id": r.id, "reference": r.reference, "amount": r.amount,
            "status": r.status, "timestamp": r.timestamp, "party": r.party
        }))
        .collect();
    Ok(Json(json!({"data": data})))
}

/// Best sellers this month by revenue.
pub async fn top_products(
    State(state): State<AppState>,
    Extension(_): Extension<AuthenticatedUser>,
) -> AppResult<Json<Value>> {
    let rows = sqlx::query!(
        r#"SELECT p.id, p.name, p.sku, SUM(sii.quantity) AS "quantity!", SUM(sii.total) AS "revenue!"
           FROM sales_invoice_items sii
           JOIN sales_invoices si ON si.id = sii.invoice_id
           JOIN products p ON p.id = sii.product_id
           WHERE si.status <> 'cancelled' AND date_trunc('month', si.invoice_date) = date_trunc('month', $1::date)
           GROUP BY p.id ORDER BY 5 DESC LIMIT 10"#,
        today()
    )
    .fetch_all(&state.pool)
    .await?;
    let data: Vec<Value> = rows
        .into_iter()
        .map(|r| json!({"id": r.id, "name": r.name, "sku": r.sku, "quantity": r.quantity, "revenue": r.revenue}))
        .collect();
    Ok(Json(json!({"data": data})))
}
