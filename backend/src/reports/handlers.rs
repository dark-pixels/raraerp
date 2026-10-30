use axum::{
    extract::{Extension, Query, State},
    Json,
};
use rust_decimal::Decimal;
use serde_json::{json, Value};

use crate::{
    auth::models::AuthenticatedUser,
    common::ListQuery,
    errors::AppResult,
    middleware::permission::require_permission,
    routes::AppState,
};

pub async fn inventory_report(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "reports.export")?;
    let rows = sqlx::query!(
        r#"SELECT p.sku, p.name, c.name AS "category?", b.name AS "brand?", p.hsn_code,
                  COALESCE(SUM(ib.on_hand), 0) AS "total_stock!",
                  COALESCE(SUM(ib.available), 0) AS "available!",
                  p.reorder_threshold, p.purchase_price, p.retail_price,
                  COALESCE(SUM(ib.on_hand), 0) * p.purchase_price AS "inventory_value!"
           FROM products p
           LEFT JOIN categories c ON c.id = p.category_id
           LEFT JOIN brands b ON b.id = p.brand_id
           LEFT JOIN inventory_balances ib ON ib.product_id = p.id
           WHERE p.status = 'active' AND ($1::uuid IS NULL OR p.category_id = $1)
           GROUP BY p.id, c.name, b.name
           HAVING NOT $2 OR COALESCE(SUM(ib.available), 0) < p.reorder_threshold
           ORDER BY p.name"#,
        q.category_id, q.low_stock.unwrap_or(false)
    )
    .fetch_all(&state.pool)
    .await?;

    let total_value: Decimal = rows.iter().map(|r| r.inventory_value).sum();
    let data: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "sku": r.sku, "name": r.name, "category": r.category, "brand": r.brand, "hsn_code": r.hsn_code,
                "total_stock": r.total_stock, "available": r.available, "reorder_threshold": r.reorder_threshold,
                "purchase_price": r.purchase_price, "retail_price": r.retail_price,
                "inventory_value": r.inventory_value.round_dp(2)
            })
        })
        .collect();
    Ok(Json(json!({"data": data, "total": data.len(), "total_value": total_value.round_dp(2)})))
}

pub async fn sales_report(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "reports.export")?;
    let rows = sqlx::query!(
        r#"SELECT si.invoice_number, si.invoice_date, c.name AS "customer_name?", c.gstin AS "customer_gstin?",
                  si.taxable_value, si.cgst_amount, si.sgst_amount, si.igst_amount,
                  si.grand_total, si.amount_paid, si.outstanding_amount, si.status
           FROM sales_invoices si LEFT JOIN customers c ON c.id = si.customer_id
           WHERE ($1::date IS NULL OR si.invoice_date >= $1) AND ($2::date IS NULL OR si.invoice_date <= $2)
             AND ($3::text IS NULL OR si.status = $3)
           ORDER BY si.invoice_date DESC, si.invoice_number DESC LIMIT 5000"#,
        q.from, q.to, q.status()
    )
    .fetch_all(&state.pool)
    .await?;

    let active = rows.iter().filter(|r| r.status != "cancelled");
    let totals = active.fold((Decimal::ZERO, Decimal::ZERO, Decimal::ZERO), |acc, r| {
        (acc.0 + r.grand_total, acc.1 + r.amount_paid, acc.2 + r.outstanding_amount)
    });
    let data: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "invoice_number": r.invoice_number, "date": r.invoice_date,
                "customer": r.customer_name, "customer_gstin": r.customer_gstin,
                "taxable_value": r.taxable_value,
                "tax": r.cgst_amount + r.sgst_amount + r.igst_amount,
                "total": r.grand_total, "paid": r.amount_paid, "outstanding": r.outstanding_amount,
                "status": r.status
            })
        })
        .collect();
    Ok(Json(json!({
        "data": data,
        "summary": {"total": totals.0, "paid": totals.1, "outstanding": totals.2}
    })))
}

pub async fn purchases_report(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "reports.export")?;
    let rows = sqlx::query!(
        r#"SELECT po.po_number, po.purchase_date, s.business_name, po.supplier_invoice_number,
                  po.taxable_value, po.gst_amount, po.grand_total, po.advance_paid,
                  po.outstanding_amount, po.status, po.payment_status
           FROM purchase_orders po JOIN suppliers s ON s.id = po.supplier_id
           WHERE ($1::date IS NULL OR po.purchase_date >= $1) AND ($2::date IS NULL OR po.purchase_date <= $2)
             AND ($3::text IS NULL OR po.status = $3)
           ORDER BY po.purchase_date DESC LIMIT 5000"#,
        q.from, q.to, q.status()
    )
    .fetch_all(&state.pool)
    .await?;

    let data: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "po_number": r.po_number, "date": r.purchase_date, "supplier": r.business_name,
                "supplier_invoice": r.supplier_invoice_number,
                "taxable_value": r.taxable_value, "gst": r.gst_amount, "total": r.grand_total,
                "paid": r.advance_paid, "outstanding": r.outstanding_amount,
                "status": r.status, "payment_status": r.payment_status
            })
        })
        .collect();
    Ok(Json(json!({"data": data})))
}

pub async fn payables_report(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "reports.export")?;
    crate::finance::handlers::list_payables(State(state), Extension(user)).await
}

pub async fn receivables_report(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "reports.export")?;
    crate::finance::handlers::list_receivables(State(state), Extension(user)).await
}

/// Output-tax summary grouped by GST rate (for GSTR-1 / GSTR-3B preparation),
/// plus input tax from purchases.
pub async fn gst_report(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "reports.export")?;
    let output = sqlx::query!(
        r#"SELECT sii.gst_rate,
                  COUNT(DISTINCT si.id) AS "invoices!",
                  SUM(sii.taxable_value) AS "taxable!",
                  SUM(sii.cgst_amount) AS "cgst!", SUM(sii.sgst_amount) AS "sgst!", SUM(sii.igst_amount) AS "igst!",
                  SUM(sii.total) AS "total!"
           FROM sales_invoice_items sii JOIN sales_invoices si ON si.id = sii.invoice_id
           WHERE si.status <> 'cancelled'
             AND ($1::date IS NULL OR si.invoice_date >= $1) AND ($2::date IS NULL OR si.invoice_date <= $2)
           GROUP BY sii.gst_rate ORDER BY sii.gst_rate"#,
        q.from, q.to
    )
    .fetch_all(&state.pool)
    .await?;

    let hsn = sqlx::query!(
        r#"SELECT COALESCE(p.hsn_code, 'N/A') AS "hsn!", sii.gst_rate,
                  SUM(sii.quantity) AS "quantity!", SUM(sii.taxable_value) AS "taxable!",
                  SUM(sii.cgst_amount + sii.sgst_amount + sii.igst_amount) AS "tax!"
           FROM sales_invoice_items sii
           JOIN sales_invoices si ON si.id = sii.invoice_id
           JOIN products p ON p.id = sii.product_id
           WHERE si.status <> 'cancelled'
             AND ($1::date IS NULL OR si.invoice_date >= $1) AND ($2::date IS NULL OR si.invoice_date <= $2)
           GROUP BY 1, 2 ORDER BY 1, 2"#,
        q.from, q.to
    )
    .fetch_all(&state.pool)
    .await?;

    let input = sqlx::query!(
        r#"SELECT COALESCE(SUM(taxable_value), 0) AS "taxable!", COALESCE(SUM(gst_amount), 0) AS "gst!"
           FROM purchase_orders
           WHERE status NOT IN ('cancelled', 'draft')
             AND ($1::date IS NULL OR purchase_date >= $1) AND ($2::date IS NULL OR purchase_date <= $2)"#,
        q.from, q.to
    )
    .fetch_one(&state.pool)
    .await?;

    let output_tax: Decimal = output.iter().map(|r| r.cgst + r.sgst + r.igst).sum();
    let data: Vec<Value> = output
        .iter()
        .map(|r| {
            json!({
                "gst_rate": r.gst_rate, "invoices": r.invoices, "taxable_value": r.taxable,
                "cgst": r.cgst, "sgst": r.sgst, "igst": r.igst,
                "total_tax": r.cgst + r.sgst + r.igst, "total": r.total
            })
        })
        .collect();

    Ok(Json(json!({
        "data": data,
        "hsn_summary": hsn.iter().map(|r| json!({
            "hsn_code": r.hsn, "gst_rate": r.gst_rate, "quantity": r.quantity,
            "taxable_value": r.taxable, "tax": r.tax
        })).collect::<Vec<_>>(),
        "output_tax": output_tax,
        "input_tax": input.gst,
        "input_taxable": input.taxable,
        "net_tax_payable": output_tax - input.gst,
    })))
}
