use crate::database::DbPool;
use tracing::{info, warn};

pub async fn process_payment_reminders(pool: &DbPool) -> Result<(), sqlx::Error> {
    // Find reminders that are due (2 days before payment due date)
    let reminders = sqlx::query!(
        r#"SELECT pr.id, pr.reference_type, pr.reference_id, pr.outstanding_amount, pr.due_date,
                  pr.reminder_days_before
           FROM payment_reminders pr
           WHERE pr.status = 'scheduled'
             AND pr.due_date - pr.reminder_days_before * INTERVAL '1 day' <= CURRENT_DATE
             AND pr.outstanding_amount > 0
             AND pr.next_reminder_at IS NULL OR pr.next_reminder_at <= NOW()"#
    ).fetch_all(pool).await?;

    info!("Processing {} payment reminders", reminders.len());

    for reminder in &reminders {
        let is_overdue = reminder.due_date.map(|d| d < chrono::Utc::now().date_naive()).unwrap_or(false);
        let title = if is_overdue {
            format!("OVERDUE: Payment of ₹{} is overdue!", reminder.outstanding_amount)
        } else {
            format!("Payment Due: ₹{} due on {:?}", reminder.outstanding_amount, reminder.due_date)
        };

        let body = match reminder.reference_type.as_str() {
            "purchase" => format!("Purchase order payment of ₹{} is {}", reminder.outstanding_amount, if is_overdue { "overdue" } else { "due soon" }),
            "invoice" => format!("Customer invoice payment of ₹{} is {}", reminder.outstanding_amount, if is_overdue { "overdue" } else { "due soon" }),
            _ => title.clone(),
        };

        // Create in-app notification for all admins
        let admin_ids = sqlx::query_scalar!(
            "SELECT id FROM users WHERE role IN ('super_admin', 'admin') AND is_active = true"
        ).fetch_all(pool).await?;

        for admin_id in &admin_ids {
            let notification_type = if is_overdue { "payment_overdue" } else { "payment_due_soon" };
            sqlx::query!(
                r#"INSERT INTO notifications (user_id, type, title, body, reference_type, reference_id)
                   VALUES ($1, $2, $3, $4, $5, $6)
                   ON CONFLICT DO NOTHING"#,
                admin_id,
                notification_type,
                title,
                body,
                reminder.reference_type,
                reminder.reference_id.to_string()
            ).execute(pool).await.ok();
        }

        // Update reminder - set next check time (daily for overdue)
        let next_check = chrono::Utc::now() + chrono::Duration::hours(24);
        let new_status = if is_overdue { "overdue" } else { "notified" };

        sqlx::query!(
            "UPDATE payment_reminders SET status = $1, next_reminder_at = $2, last_notified_at = NOW() WHERE id = $3",
            new_status, next_check, reminder.id
        ).execute(pool).await?;

        info!("Processed reminder {} ({})", reminder.id, new_status);
    }

    Ok(())
}
