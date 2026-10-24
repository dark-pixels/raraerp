use crate::database::DbPool;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{info, error, warn};
use rust_decimal::Decimal;

pub mod reminder;

pub async fn start_scheduler(pool: DbPool) {
    info!("Starting background job scheduler");

    let scheduler = JobScheduler::new().await.expect("Failed to create scheduler");

    // Check payment reminders every 30 minutes
    let pool_clone = pool.clone();
    let reminder_job = Job::new_async("0 */30 * * * *", move |_uuid, _l| {
        let pool = pool_clone.clone();
        Box::pin(async move {
            info!("Running payment reminder check");
            if let Err(e) = reminder::process_payment_reminders(&pool).await {
                error!("Payment reminder error: {}", e);
            }
        })
    }).expect("Failed to create reminder job");

    // Mark overdue invoices every hour
    let pool_clone2 = pool.clone();
    let overdue_job = Job::new_async("0 0 * * * *", move |_uuid, _l| {
        let pool = pool_clone2.clone();
        Box::pin(async move {
            info!("Running overdue update");
            if let Err(e) = mark_overdue(&pool).await {
                error!("Overdue update error: {}", e);
            }
        })
    }).expect("Failed to create overdue job");

    scheduler.add(reminder_job).await.expect("Failed to add reminder job");
    scheduler.add(overdue_job).await.expect("Failed to add overdue job");

    scheduler.start().await.expect("Failed to start scheduler");

    info!("Background scheduler running. Press Ctrl+C to stop.");

    // Keep the process alive
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}

async fn mark_overdue(pool: &DbPool) -> Result<(), sqlx::Error> {
    // Mark overdue sales invoices
    sqlx::query!(
        "UPDATE sales_invoices SET status = 'overdue' WHERE due_date < CURRENT_DATE AND outstanding_amount > 0 AND status IN ('issued', 'partially_paid')"
    ).execute(pool).await?;

    // Mark overdue purchase orders
    sqlx::query!(
        "UPDATE purchase_orders SET payment_status = 'overdue' WHERE due_date < CURRENT_DATE AND outstanding_amount > 0 AND payment_status IN ('unpaid', 'partially_paid')"
    ).execute(pool).await?;

    Ok(())
}
