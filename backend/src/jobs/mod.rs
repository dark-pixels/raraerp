use serde_json::{json, Value};
use tracing::{error, info};

use crate::database::DbPool;

pub mod reminder;

/// Marks invoices and purchase orders past their due date as overdue.
pub async fn mark_overdue(pool: &DbPool) -> Result<(u64, u64), sqlx::Error> {
    let today = crate::common::today();
    let invoices = sqlx::query!(
        "UPDATE sales_invoices SET status = 'overdue', updated_at = NOW() WHERE due_date < $1 AND outstanding_amount > 0 AND status IN ('issued', 'partially_paid')",
        today
    )
    .execute(pool)
    .await?
    .rows_affected();

    let purchases = sqlx::query!(
        "UPDATE purchase_orders SET payment_status = 'overdue', updated_at = NOW() WHERE due_date < $1 AND outstanding_amount > 0 AND payment_status IN ('unpaid', 'partially_paid') AND status <> 'cancelled'",
        today
    )
    .execute(pool)
    .await?
    .rows_affected();

    Ok((invoices, purchases))
}

/// Runs every scheduled job once and reports what each did.
pub async fn run_all(pool: &DbPool) -> Value {
    let overdue = mark_overdue(pool).await;
    let reminders = reminder::process_payment_reminders(pool).await;
    let low_stock = reminder::process_low_stock_alerts(pool).await;

    let fmt = |r: Result<String, sqlx::Error>| match r {
        Ok(v) => v,
        Err(e) => {
            error!("Scheduled job failed: {}", e);
            format!("error: {}", e)
        }
    };
    json!({
        "overdue": fmt(overdue.map(|(i, p)| format!("{} invoices, {} purchases", i, p))),
        "reminders": fmt(reminders.map(|n| n.to_string())),
        "low_stock_alerts": fmt(low_stock.map(|n| n.to_string())),
    })
}

/// Long-running scheduler used by the standalone worker binary.
#[allow(dead_code)] // unused by the server binary
pub async fn start_scheduler(pool: DbPool) {
    use tokio_cron_scheduler::{Job, JobScheduler};
    info!("Starting background job scheduler");

    let scheduler = match JobScheduler::new().await {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to create scheduler: {}", e);
            return;
        }
    };

    // Every 30 minutes.
    let job_pool = pool.clone();
    let job = Job::new_async("0 */30 * * * *", move |_uuid, _l| {
        let pool = job_pool.clone();
        Box::pin(async move {
            let result = run_all(&pool).await;
            info!("Scheduled jobs completed: {}", result);
        })
    });

    match job {
        Ok(job) => {
            if let Err(e) = scheduler.add(job).await {
                error!("Failed to add job: {}", e);
                return;
            }
        }
        Err(e) => {
            error!("Failed to create job: {}", e);
            return;
        }
    }

    if let Err(e) = scheduler.start().await {
        error!("Failed to start scheduler: {}", e);
        return;
    }

    // Run once at startup so a freshly started worker is immediately useful.
    info!("Initial run: {}", run_all(&pool).await);

    let _ = tokio::signal::ctrl_c().await;
    info!("Worker shutting down");
}
