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
    common::{dec_field, money, paginated, payment_method, required_text, text, today, ListQuery},
    errors::{AppError, AppResult},
    middleware::permission::{has_permission, require_permission},
    routes::AppState,
};

pub async fn list_payables(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "finance.read")?;
    let rows = sqlx::query!(
        r#"SELECT po.id, po.po_number, po.purchase_date, po.grand_total, po.advance_paid, po.outstanding_amount,
                  po.due_date, po.payment_status, po.supplier_id, s.business_name AS supplier_name,
                  (po.due_date < $1) AS "is_overdue!"
           FROM purchase_orders po JOIN suppliers s ON s.id = po.supplier_id
           WHERE po.outstanding_amount > 0 AND po.status <> 'cancelled'
           ORDER BY po.due_date ASC NULLS LAST"#,
        today()
    )
    .fetch_all(&state.pool)
    .await?;

    let total: Decimal = rows.iter().map(|r| r.outstanding_amount).sum();
    let overdue: Decimal = rows.iter().filter(|r| r.is_overdue).map(|r| r.outstanding_amount).sum();
    let data: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.id, "po_number": r.po_number, "purchase_date": r.purchase_date,
                "supplier_id": r.supplier_id, "supplier_name": r.supplier_name,
                "grand_total": r.grand_total, "amount_paid": r.advance_paid,
                "outstanding_amount": r.outstanding_amount, "outstanding": r.outstanding_amount,
                "due_date": r.due_date, "payment_status": r.payment_status, "status": r.payment_status,
                "is_overdue": r.is_overdue,
            })
        })
        .collect();

    Ok(Json(json!({"data": data, "total_payable": total, "overdue_payable": overdue})))
}

pub async fn list_receivables(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "finance.read")?;
    let rows = sqlx::query!(
        r#"SELECT si.id, si.invoice_number, si.invoice_date, si.grand_total, si.amount_paid, si.outstanding_amount,
                  si.due_date, si.status, si.customer_id,
                  c.name AS "customer_name?", c.phone AS "customer_phone?",
                  (si.due_date < $1) AS "is_overdue!"
           FROM sales_invoices si LEFT JOIN customers c ON c.id = si.customer_id
           WHERE si.outstanding_amount > 0 AND si.status <> 'cancelled'
           ORDER BY si.due_date ASC NULLS LAST"#,
        today()
    )
    .fetch_all(&state.pool)
    .await?;

    let total: Decimal = rows.iter().map(|r| r.outstanding_amount).sum();
    let overdue: Decimal = rows.iter().filter(|r| r.is_overdue).map(|r| r.outstanding_amount).sum();
    let data: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.id, "invoice_number": r.invoice_number, "invoice_date": r.invoice_date,
                "customer_id": r.customer_id, "customer_name": r.customer_name, "customer_phone": r.customer_phone,
                "grand_total": r.grand_total, "amount_paid": r.amount_paid,
                "outstanding_amount": r.outstanding_amount, "outstanding": r.outstanding_amount,
                "due_date": r.due_date, "status": r.status, "is_overdue": r.is_overdue,
            })
        })
        .collect();

    Ok(Json(json!({"data": data, "total_receivable": total, "overdue_receivable": overdue})))
}

pub async fn list_expenses(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "finance.read")?;
    let status = q.status();
    let search = q.search();
    let rows = sqlx::query!(
        r#"SELECT e.id, e.category, e.amount, e.date, e.payment_method, e.reference, e.description,
                  e.approval_status, e.created_at, e.created_by, u.full_name AS "created_by_name?", a.full_name AS "approved_by_name?",
                  COUNT(*) OVER () AS "full_count!"
           FROM expenses e
           LEFT JOIN users u ON u.id = e.created_by
           LEFT JOIN users a ON a.id = e.approved_by
           WHERE ($1::text IS NULL OR e.approval_status = $1)
             AND ($2::text IS NULL OR e.category ILIKE '%' || $2 || '%' OR e.description ILIKE '%' || $2 || '%')
             AND ($3::date IS NULL OR e.date >= $3)
             AND ($4::date IS NULL OR e.date <= $4)
           ORDER BY e.date DESC, e.created_at DESC
           LIMIT $5 OFFSET $6"#,
        status, search, q.from, q.to, q.per_page(), q.offset()
    )
    .fetch_all(&state.pool)
    .await?;

    let total = rows.first().map(|r| r.full_count).unwrap_or(0);
    let data = rows
        .into_iter()
        .map(|r| {
            json!({
                "id": r.id, "category": r.category, "amount": r.amount,
                "date": r.date, "expense_date": r.date,
                "payment_method": r.payment_method, "method": r.payment_method,
                "reference": r.reference, "description": r.description, "notes": r.description,
                "approval_status": r.approval_status, "status": r.approval_status,
                "created_by_id": r.created_by, "created_by_name": r.created_by_name, "approved_by_name": r.approved_by_name,
                "created_at": r.created_at,
            })
        })
        .collect();
    Ok(Json(paginated(data, total, &q)))
}

pub async fn create_expense(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(body): Json<Value>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "finance.read")?;
    let category = required_text(&body, "category")?;
    let amount = money(dec_field(&body, "amount")?.ok_or_else(|| AppError::Validation("Amount is required".into()))?);
    if amount <= Decimal::ZERO {
        return Err(AppError::Validation("Amount must be greater than zero".into()));
    }
    let date = match text(body.get("date")) {
        Some(d) => d.parse::<chrono::NaiveDate>().map_err(|_| AppError::Validation("Date must be YYYY-MM-DD".into()))?,
        None => today(),
    };
    if date > today() {
        return Err(AppError::Validation("Expense date cannot be in the future".into()));
    }
    let method = payment_method(body.get("payment_method"))?;

    // Users who can approve payments have their own expenses approved automatically.
    let auto_approve = has_permission(&user, "payments.approve");
    let status = if auto_approve { "approved" } else { "pending" };
    let approver = if auto_approve { Some(user.id) } else { None };

    let id = sqlx::query_scalar!(
        r#"INSERT INTO expenses (category, amount, date, payment_method, reference, description, created_by, approved_by, approval_status)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9) RETURNING id"#,
        category, amount, date, method, text(body.get("reference")),
        text(body.get("description")).or_else(|| text(body.get("notes"))),
        user.id, approver, status
    )
    .fetch_one(&state.pool)
    .await?;

    log_action(&state.pool, user.id, "expense_created", "expense", Some(&id.to_string()),
        Some(json!({"category": category, "amount": amount}))).await;
    Ok(Json(json!({"id": id, "approval_status": status, "message": "Expense recorded"})))
}

pub async fn review_expense(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path((id, decision)): Path<(Uuid, String)>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "payments.approve")?;
    let status = match decision.as_str() {
        "approve" => "approved",
        "reject" => "rejected",
        _ => return Err(AppError::NotFound("Unknown action".into())),
    };
    let res = sqlx::query!(
        "UPDATE expenses SET approval_status = $1, approved_by = $2 WHERE id = $3 AND approval_status = 'pending'",
        status, user.id, id
    )
    .execute(&state.pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::BusinessLogic("Only pending expenses can be reviewed".into()));
    }
    log_action(&state.pool, user.id, &format!("expense_{}", status), "expense", Some(&id.to_string()), None).await;
    Ok(Json(json!({"message": format!("Expense {}", status)})))
}

pub async fn financial_summary(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "finance.read")?;
    let row = sqlx::query!(
        r#"SELECT
             (SELECT COALESCE(SUM(grand_total), 0) FROM sales_invoices
                WHERE status NOT IN ('cancelled') AND ($1::date IS NULL OR invoice_date >= $1) AND ($2::date IS NULL OR invoice_date <= $2)) AS "sales!",
             (SELECT COALESCE(SUM(taxable_value), 0) FROM sales_invoices
                WHERE status NOT IN ('cancelled') AND ($1::date IS NULL OR invoice_date >= $1) AND ($2::date IS NULL OR invoice_date <= $2)) AS "sales_taxable!",
             (SELECT COALESCE(SUM(total), 0) FROM credit_notes
                WHERE status <> 'cancelled' AND ($1::date IS NULL OR created_at >= $1) AND ($2::date IS NULL OR created_at < $2 + 1)) AS "returns!",
             (SELECT COALESCE(SUM(grand_total), 0) FROM purchase_orders
                WHERE status NOT IN ('cancelled', 'draft') AND ($1::date IS NULL OR purchase_date >= $1) AND ($2::date IS NULL OR purchase_date <= $2)) AS "purchases!",
             (SELECT COALESCE(SUM(amount), 0) FROM expenses
                WHERE approval_status = 'approved' AND ($1::date IS NULL OR date >= $1) AND ($2::date IS NULL OR date <= $2)) AS "expenses!",
             (SELECT COALESCE(SUM(amount), 0) FROM expenses WHERE approval_status = 'pending') AS "pending_expenses!",
             (SELECT COALESCE(SUM(amount), 0) FROM payments
                WHERE direction = 'inbound' AND status = 'verified' AND ($1::date IS NULL OR payment_date >= $1) AND ($2::date IS NULL OR payment_date <= $2)) AS "collected!",
             (SELECT COALESCE(SUM(amount), 0) FROM payments
                WHERE direction = 'outbound' AND status = 'verified' AND ($1::date IS NULL OR payment_date >= $1) AND ($2::date IS NULL OR payment_date <= $2)) AS "paid_out!",
             (SELECT COALESCE(SUM(outstanding_amount), 0) FROM sales_invoices WHERE outstanding_amount > 0 AND status <> 'cancelled') AS "receivables!",
             (SELECT COALESCE(SUM(outstanding_amount), 0) FROM purchase_orders WHERE outstanding_amount > 0 AND status <> 'cancelled') AS "payables!",
             (SELECT COALESCE(SUM(sii.quantity * p.purchase_price), 0)
                FROM sales_invoice_items sii
                JOIN sales_invoices si ON si.id = sii.invoice_id
                JOIN products p ON p.id = sii.product_id
                WHERE si.status NOT IN ('cancelled') AND ($1::date IS NULL OR si.invoice_date >= $1) AND ($2::date IS NULL OR si.invoice_date <= $2)) AS "cogs!""#,
        q.from, q.to
    )
    .fetch_one(&state.pool)
    .await?;

    // Gross profit uses current cost prices as an estimate of cost of goods sold.
    let gross_profit = row.sales_taxable - row.cogs;
    let net_profit = gross_profit - row.expenses;

    Ok(Json(json!({
        "total_sales": row.sales,
        "net_sales": row.sales_taxable,
        "returns": row.returns,
        "total_purchases": row.purchases,
        "total_expenses": row.expenses,
        "pending_expenses": row.pending_expenses,
        "collected": row.collected,
        "paid_out": row.paid_out,
        "net_cash_flow": row.collected - row.paid_out - row.expenses,
        "cost_of_goods_sold": money(row.cogs),
        "gross_profit": money(gross_profit),
        "net_profit": money(net_profit),
        "receivables": row.receivables,
        "payables": row.payables,
    })))
}

/// Only payment approvers may change approved/rejected expenses; anyone else may
/// change their own expense while it is still pending.
async fn editable_expense(state: &AppState, user: &AuthenticatedUser, id: Uuid) -> AppResult<String> {
    let row = sqlx::query!("SELECT approval_status, created_by FROM expenses WHERE id = $1", id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Expense not found".into()))?;
    let approver = has_permission(user, "payments.approve");
    if !approver && (row.approval_status != "pending" || row.created_by != Some(user.id)) {
        return Err(AppError::Forbidden(
            "You can only change your own expenses while they are pending approval".into(),
        ));
    }
    Ok(row.approval_status)
}

fn expense_date(body: &Value) -> AppResult<Option<chrono::NaiveDate>> {
    match text(body.get("date")).or_else(|| text(body.get("expense_date"))) {
        Some(d) => {
            let date = d
                .parse::<chrono::NaiveDate>()
                .map_err(|_| AppError::Validation("Date must be YYYY-MM-DD".into()))?;
            if date > today() {
                return Err(AppError::Validation("Expense date cannot be in the future".into()));
            }
            Ok(Some(date))
        }
        None => Ok(None),
    }
}

pub async fn update_expense(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
    Json(body): Json<Value>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "finance.read")?;
    editable_expense(&state, &user, id).await?;

    let category = text(body.get("category"));
    let amount = dec_field(&body, "amount")?.map(money);
    if matches!(amount, Some(a) if a <= Decimal::ZERO) {
        return Err(AppError::Validation("Amount must be greater than zero".into()));
    }
    let date = expense_date(&body)?;
    let method = match body.get("payment_method") {
        Some(v) if !v.is_null() => Some(payment_method(Some(v))?),
        _ => None,
    };
    // Empty strings clear the optional text fields; missing keys leave them unchanged.
    let reference = body.get("reference").map(|v| text(Some(v)));
    let description = body.get("description").or_else(|| body.get("notes")).map(|v| text(Some(v)));

    sqlx::query!(
        r#"UPDATE expenses SET
             category = COALESCE($1, category),
             amount = COALESCE($2, amount),
             date = COALESCE($3, date),
             payment_method = COALESCE($4, payment_method),
             reference = CASE WHEN $5 THEN $6 ELSE reference END,
             description = CASE WHEN $7 THEN $8 ELSE description END
           WHERE id = $9"#,
        category, amount, date, method,
        reference.is_some(), reference.clone().flatten(),
        description.is_some(), description.clone().flatten(),
        id
    )
    .execute(&state.pool)
    .await?;

    log_action(&state.pool, user.id, "expense_updated", "expense", Some(&id.to_string()), Some(body)).await;
    Ok(Json(json!({"message": "Expense updated"})))
}

pub async fn delete_expense(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "finance.read")?;
    editable_expense(&state, &user, id).await?;
    let old = sqlx::query!(
        "DELETE FROM expenses WHERE id = $1 RETURNING category, amount, date",
        id
    )
    .fetch_one(&state.pool)
    .await?;
    log_action(&state.pool, user.id, "expense_deleted", "expense", Some(&id.to_string()),
        Some(json!({"category": old.category, "amount": old.amount, "date": old.date}))).await;
    Ok(Json(json!({"message": "Expense deleted"})))
}

/// Changes the due date of an unpaid sales invoice or purchase order.
pub async fn update_due_date(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path((kind, id)): Path<(String, Uuid)>,
    Json(body): Json<Value>,
) -> AppResult<Json<Value>> {
    require_permission(&user, "payments.approve")?;
    let due = text(body.get("due_date"))
        .ok_or_else(|| AppError::Validation("Due date is required".into()))?
        .parse::<chrono::NaiveDate>()
        .map_err(|_| AppError::Validation("Due date must be YYYY-MM-DD".into()))?;

    let (res, reference_type) = match kind.as_str() {
        "invoice" => (
            sqlx::query!(
                "UPDATE sales_invoices SET due_date = $1, updated_at = NOW()
                 WHERE id = $2 AND outstanding_amount > 0 AND status <> 'cancelled' AND $1 >= invoice_date",
                due, id
            )
            .execute(&state.pool)
            .await?,
            "invoice",
        ),
        "purchase" => (
            sqlx::query!(
                "UPDATE purchase_orders SET due_date = $1, updated_at = NOW()
                 WHERE id = $2 AND outstanding_amount > 0 AND status <> 'cancelled' AND $1 >= purchase_date",
                due, id
            )
            .execute(&state.pool)
            .await?,
            "purchase",
        ),
        _ => return Err(AppError::NotFound("Unknown document type".into())),
    };
    if res.rows_affected() == 0 {
        return Err(AppError::BusinessLogic(
            "Only unpaid documents can be changed, and the due date cannot be before the document date".into(),
        ));
    }

    // Reschedule open reminders so they follow the new date.
    sqlx::query!(
        "UPDATE payment_reminders SET due_date = $1, status = 'scheduled', last_notified_at = NULL
         WHERE reference_type = $2 AND reference_id = $3 AND status IN ('scheduled', 'notified', 'overdue')",
        due, reference_type, id
    )
    .execute(&state.pool)
    .await?;

    log_action(&state.pool, user.id, "due_date_changed", reference_type, Some(&id.to_string()),
        Some(json!({"due_date": due}))).await;
    Ok(Json(json!({"message": "Due date updated"})))
}
