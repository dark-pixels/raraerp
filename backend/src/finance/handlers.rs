use axum::{extract::{State, Extension}, Json};
use rust_decimal::Decimal;
use crate::{auth::models::AuthenticatedUser, errors::AppResult, middleware::permission::require_permission, routes::AppState};

pub async fn list_payables(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "finance.read")?;
    let rows = sqlx::query!(
        r#"SELECT po.id, po.po_number, po.outstanding_amount, po.due_date, po.payment_status,
                  s.business_name as supplier_name
           FROM purchase_orders po JOIN suppliers s ON s.id = po.supplier_id
           WHERE po.outstanding_amount > 0
           ORDER BY po.due_date ASC NULLS LAST"#
    ).fetch_all(&state.pool).await?;

    let data: Vec<_> = rows.iter().map(|r| serde_json::json!({
        "id": r.id, "po_number": r.po_number, "supplier_name": r.supplier_name,
        "outstanding": r.outstanding_amount, "due_date": r.due_date, "status": r.payment_status
    })).collect();

    let total: Option<Decimal> = sqlx::query_scalar!("SELECT SUM(outstanding_amount) FROM purchase_orders WHERE outstanding_amount > 0")
        .fetch_one(&state.pool).await?;

    Ok(Json(serde_json::json!({"data": data, "total_payable": total.unwrap_or(Decimal::ZERO)})))
}

pub async fn list_receivables(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "finance.read")?;
    let rows = sqlx::query!(
        r#"SELECT si.id, si.invoice_number, si.outstanding_amount, si.due_date, si.status,
                  c.name as "customer_name?"
           FROM sales_invoices si LEFT JOIN customers c ON c.id = si.customer_id
           WHERE si.outstanding_amount > 0 AND si.status NOT IN ('cancelled')
           ORDER BY si.due_date ASC NULLS LAST"#
    ).fetch_all(&state.pool).await?;

    let data: Vec<_> = rows.iter().map(|r| serde_json::json!({
        "id": r.id, "invoice_number": r.invoice_number, "customer_name": r.customer_name,
        "outstanding": r.outstanding_amount, "due_date": r.due_date, "status": r.status
    })).collect();

    let total: Option<Decimal> = sqlx::query_scalar!("SELECT SUM(outstanding_amount) FROM sales_invoices WHERE outstanding_amount > 0 AND status != 'cancelled'")
        .fetch_one(&state.pool).await?;

    Ok(Json(serde_json::json!({"data": data, "total_receivable": total.unwrap_or(Decimal::ZERO)})))
}

pub async fn list_expenses(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "finance.read")?;
    let rows = sqlx::query!(
        "SELECT id, category, amount, date, payment_method, description, approval_status FROM expenses ORDER BY date DESC LIMIT 100"
    ).fetch_all(&state.pool).await?;
    let data: Vec<_> = rows.iter().map(|r| serde_json::json!({
        "id": r.id, "category": r.category, "amount": r.amount, "date": r.date,
        "method": r.payment_method, "description": r.description, "status": r.approval_status
    })).collect();
    Ok(Json(serde_json::json!({"data": data})))
}

pub async fn create_expense(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "finance.read")?;
    let cat = body.get("category").and_then(|v| v.as_str()).unwrap_or("Miscellaneous");
    let amount: Decimal = body.get("amount").and_then(|v| v.as_str()).and_then(|s| s.parse().ok())
        .ok_or_else(|| crate::errors::AppError::Validation("amount required".into()))?;
    let id = sqlx::query_scalar!(
        r#"INSERT INTO expenses (category, amount, date, payment_method, description, created_by)
           VALUES ($1,$2,CURRENT_DATE,$3,$4,$5) RETURNING id"#,
        cat, amount,
        body.get("payment_method").and_then(|v| v.as_str()).unwrap_or("cash"),
        body.get("description").and_then(|v| v.as_str()),
        user.id
    ).fetch_one(&state.pool).await?;
    Ok(Json(serde_json::json!({"id": id})))
}

pub async fn financial_summary(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(&user, "finance.read")?;
    let total_sales: Option<Decimal> = sqlx::query_scalar!("SELECT SUM(grand_total) FROM sales_invoices WHERE status != 'cancelled'").fetch_one(&state.pool).await?;
    let total_purchases: Option<Decimal> = sqlx::query_scalar!("SELECT SUM(grand_total) FROM purchase_orders WHERE status != 'cancelled'").fetch_one(&state.pool).await?;
    let total_expenses: Option<Decimal> = sqlx::query_scalar!("SELECT SUM(amount) FROM expenses WHERE approval_status = 'approved'").fetch_one(&state.pool).await?;
    let receivables: Option<Decimal> = sqlx::query_scalar!("SELECT SUM(outstanding_amount) FROM sales_invoices WHERE outstanding_amount > 0").fetch_one(&state.pool).await?;
    let payables: Option<Decimal> = sqlx::query_scalar!("SELECT SUM(outstanding_amount) FROM purchase_orders WHERE outstanding_amount > 0").fetch_one(&state.pool).await?;

    Ok(Json(serde_json::json!({
        "total_sales": total_sales.unwrap_or(Decimal::ZERO),
        "total_purchases": total_purchases.unwrap_or(Decimal::ZERO),
        "total_expenses": total_expenses.unwrap_or(Decimal::ZERO),
        "receivables": receivables.unwrap_or(Decimal::ZERO),
        "payables": payables.unwrap_or(Decimal::ZERO),
    })))
}
