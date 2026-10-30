use tracing::info;

use crate::database::DbPool;

/// Notifies admins about payments due soon or overdue. Each reminder fires at
/// most once a day and stops once its document is settled.
pub async fn process_payment_reminders(pool: &DbPool) -> Result<usize, sqlx::Error> {
    let today = crate::common::today();

    // Settle reminders whose document has since been paid or cancelled.
    sqlx::query!(
        r#"UPDATE payment_reminders pr SET status = 'settled'
           WHERE pr.status IN ('scheduled', 'notified', 'overdue')
             AND (
               (pr.reference_type = 'invoice' AND NOT EXISTS (
                   SELECT 1 FROM sales_invoices si WHERE si.id = pr.reference_id AND si.outstanding_amount > 0 AND si.status <> 'cancelled'))
               OR
               (pr.reference_type = 'purchase' AND NOT EXISTS (
                   SELECT 1 FROM purchase_orders po WHERE po.id = pr.reference_id AND po.outstanding_amount > 0 AND po.status <> 'cancelled'))
             )"#
    )
    .execute(pool)
    .await?;

    let reminders = sqlx::query!(
        r#"SELECT pr.id, pr.reference_type, pr.reference_id, pr.due_date,
                  COALESCE(si.outstanding_amount, po.outstanding_amount, pr.outstanding_amount) AS "outstanding!",
                  COALESCE(si.invoice_number, po.po_number) AS "document?",
                  COALESCE(c.name, s.business_name) AS "party?"
           FROM payment_reminders pr
           LEFT JOIN sales_invoices si ON pr.reference_type = 'invoice' AND si.id = pr.reference_id
           LEFT JOIN customers c ON c.id = si.customer_id
           LEFT JOIN purchase_orders po ON pr.reference_type = 'purchase' AND po.id = pr.reference_id
           LEFT JOIN suppliers s ON s.id = po.supplier_id
           WHERE pr.status IN ('scheduled', 'notified', 'overdue')
             AND pr.due_date IS NOT NULL
             AND pr.due_date - pr.reminder_days_before <= $1
             AND (pr.next_reminder_at IS NULL OR pr.next_reminder_at <= NOW())"#,
        today
    )
    .fetch_all(pool)
    .await?;

    if reminders.is_empty() {
        return Ok(0);
    }

    let admin_ids = sqlx::query_scalar!("SELECT id FROM users WHERE role IN ('super_admin', 'admin') AND is_active = true")
        .fetch_all(pool)
        .await?;

    for r in &reminders {
        let is_overdue = r.due_date.map(|d| d < today).unwrap_or(false);
        let amount = crate::common::format_inr(r.outstanding);
        let doc = r.document.clone().unwrap_or_else(|| "document".into());
        let party = r.party.clone().unwrap_or_default();
        let due = r.due_date.map(|d| d.format("%d %b %Y").to_string()).unwrap_or_default();

        let (kind, title, body) = match (r.reference_type.as_str(), is_overdue) {
            ("invoice", true) => ("payment_overdue", format!("Overdue: {} from {}", doc, party),
                format!("Rs. {} was due on {} and has not been received.", amount, due)),
            ("invoice", false) => ("payment_due_soon", format!("Payment due: {} from {}", doc, party),
                format!("Rs. {} is due from the customer on {}.", amount, due)),
            (_, true) => ("payment_overdue", format!("Overdue: pay {} for {}", party, doc),
                format!("Rs. {} was due to the supplier on {}.", amount, due)),
            (_, false) => ("payment_due_soon", format!("Upcoming: pay {} for {}", party, doc),
                format!("Rs. {} is due to the supplier on {}.", amount, due)),
        };

        for admin_id in &admin_ids {
            sqlx::query!(
                r#"INSERT INTO notifications (user_id, type, title, body, reference_type, reference_id)
                   VALUES ($1, $2, $3, $4, $5, $6)"#,
                admin_id, kind, title, body, r.reference_type, r.reference_id.to_string()
            )
            .execute(pool)
            .await?;
        }

        sqlx::query!(
            r#"UPDATE payment_reminders SET status = $1, next_reminder_at = NOW() + INTERVAL '24 hours',
                   last_notified_at = NOW(), outstanding_amount = $2
               WHERE id = $3"#,
            if is_overdue { "overdue" } else { "notified" }, r.outstanding, r.id
        )
        .execute(pool)
        .await?;
    }

    info!("Sent {} payment reminders", reminders.len());
    Ok(reminders.len())
}

/// Notifies admins once per day about products below their reorder level.
pub async fn process_low_stock_alerts(pool: &DbPool) -> Result<usize, sqlx::Error> {
    let already_sent: bool = sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM notifications WHERE type = 'low_stock' AND created_at > NOW() - INTERVAL '20 hours') AS "e!""#
    )
    .fetch_one(pool)
    .await?;
    if already_sent {
        return Ok(0);
    }

    let low: Vec<String> = sqlx::query_scalar!(
        r#"SELECT p.name AS "name!" FROM products p
           LEFT JOIN inventory_balances ib ON ib.product_id = p.id
           WHERE p.status = 'active'
           GROUP BY p.id
           HAVING COALESCE(SUM(ib.available), 0) < p.reorder_threshold
           ORDER BY p.name"#
    )
    .fetch_all(pool)
    .await?;
    if low.is_empty() {
        return Ok(0);
    }

    let preview = low.iter().take(5).cloned().collect::<Vec<_>>().join(", ");
    let more = if low.len() > 5 { format!(" and {} more", low.len() - 5) } else { String::new() };
    let title = format!("{} products are low on stock", low.len());
    let body = format!("{}{}. Review the low-stock list and raise purchase orders.", preview, more);

    sqlx::query!(
        r#"INSERT INTO notifications (user_id, type, title, body, reference_type)
           SELECT id, 'low_stock', $1, $2, 'inventory' FROM users
           WHERE role IN ('super_admin', 'admin') AND is_active = true"#,
        title, body
    )
    .execute(pool)
    .await?;
    Ok(low.len())
}
