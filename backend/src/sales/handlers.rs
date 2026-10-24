use axum::{extract::{State, Extension}, Json};
use rust_decimal::Decimal;
use crate::{auth::models::AuthenticatedUser, errors::AppResult, routes::AppState};

pub async fn dashboard_summary(
    State(state): State<AppState>,
    Extension(_user): Extension<AuthenticatedUser>,
) -> AppResult<Json<serde_json::Value>> {
    let total_products: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM products WHERE status = 'active'")
        .fetch_one(&state.pool).await?.unwrap_or(0);

    let today_sales: Option<Decimal> = sqlx::query_scalar!(
        "SELECT COALESCE(SUM(grand_total), 0) FROM sales_invoices WHERE DATE(invoice_date) = CURRENT_DATE"
    ).fetch_one(&state.pool).await?;

    let monthly_sales: Option<Decimal> = sqlx::query_scalar!(
        "SELECT COALESCE(SUM(grand_total), 0) FROM sales_invoices WHERE DATE_TRUNC('month', invoice_date) = DATE_TRUNC('month', CURRENT_DATE)"
    ).fetch_one(&state.pool).await?;

    let customer_receivables: Option<Decimal> = sqlx::query_scalar!(
        "SELECT COALESCE(SUM(outstanding_balance), 0) FROM customers"
    ).fetch_one(&state.pool).await?;

    let supplier_payables: Option<Decimal> = sqlx::query_scalar!(
        "SELECT COALESCE(SUM(outstanding_balance), 0) FROM suppliers"
    ).fetch_one(&state.pool).await?;

    let low_stock: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*)::bigint as "count!"
           FROM (
               SELECT p.id
               FROM products p
               LEFT JOIN inventory_balances ib ON ib.product_id = p.id
               WHERE p.status = 'active'
               GROUP BY p.id, p.reorder_threshold
               HAVING COALESCE(SUM(ib.available), 0) < p.reorder_threshold
           ) sub"#
    ).fetch_one(&state.pool).await.unwrap_or(0);

    let overdue_invoices: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM sales_invoices WHERE due_date < CURRENT_DATE AND status IN ('issued', 'partially_paid')"
    ).fetch_one(&state.pool).await?.unwrap_or(0);

    let today_collected: Option<Decimal> = sqlx::query_scalar!(
        "SELECT COALESCE(SUM(amount), 0) FROM payments WHERE DATE(payment_date) = CURRENT_DATE AND direction = 'inbound' AND status = 'verified'"
    ).fetch_one(&state.pool).await?;

    Ok(Json(serde_json::json!({
        "total_products": total_products,
        "today_sales": today_sales.unwrap_or(Decimal::ZERO),
        "monthly_sales": monthly_sales.unwrap_or(Decimal::ZERO),
        "customer_receivables": customer_receivables.unwrap_or(Decimal::ZERO),
        "supplier_payables": supplier_payables.unwrap_or(Decimal::ZERO),
        "low_stock_count": low_stock,
        "overdue_customer_invoices": overdue_invoices,
        "today_collected": today_collected.unwrap_or(Decimal::ZERO),
    })))
}

pub async fn sales_trend(
    State(state): State<AppState>,
    Extension(_): Extension<AuthenticatedUser>,
) -> AppResult<Json<serde_json::Value>> {
    let rows = sqlx::query!(
        r#"SELECT (invoice_date::date) as "date!", SUM(grand_total) as "total!"
           FROM sales_invoices
           WHERE invoice_date >= CURRENT_DATE - INTERVAL '30 days'
           GROUP BY invoice_date::date
           ORDER BY invoice_date::date"#
    ).fetch_all(&state.pool).await?;

    let data: Vec<_> = rows.iter().map(|r| serde_json::json!({"date": r.date, "total": r.total})).collect();
    Ok(Json(serde_json::json!({"data": data})))
}

pub async fn activity_feed(
    State(state): State<AppState>,
    Extension(_): Extension<AuthenticatedUser>,
) -> AppResult<Json<serde_json::Value>> {
    let invoices = sqlx::query!(
        r#"SELECT invoice_number as "reference!", grand_total as "amount!", created_at as "created_at!"
           FROM sales_invoices ORDER BY created_at DESC LIMIT 10"#
    ).fetch_all(&state.pool).await?;

    let purchases = sqlx::query!(
        r#"SELECT po_number as "reference!", grand_total as "amount!", created_at as "created_at!"
           FROM purchase_orders ORDER BY created_at DESC LIMIT 10"#
    ).fetch_all(&state.pool).await?;

    let mut activities: Vec<serde_json::Value> = invoices.iter().map(|r| serde_json::json!({
        "type": "invoice", "reference": r.reference, "amount": r.amount, "timestamp": r.created_at
    })).collect();

    for r in &purchases {
        activities.push(serde_json::json!({
            "type": "purchase", "reference": r.reference, "amount": r.amount, "timestamp": r.created_at
        }));
    }

    activities.sort_by(|a, b| {
        let ta = a.get("timestamp").and_then(|v| v.as_str()).unwrap_or("");
        let tb = b.get("timestamp").and_then(|v| v.as_str()).unwrap_or("");
        tb.cmp(ta)
    });

    let len = activities.len().min(20);
    Ok(Json(serde_json::json!({"data": &activities[..len]})))
}
