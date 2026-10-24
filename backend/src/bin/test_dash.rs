use sqlx::postgres::PgPoolOptions;
use rust_decimal::Decimal;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPoolOptions::new().connect(&url).await?;
    
    println!("Testing Q1...");
    let q1 = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM products WHERE status = 'active'").fetch_one(&pool).await;
    println!("Q1: {:?}", q1);

    println!("Testing Q2...");
    let q2 = sqlx::query_scalar::<_, Option<Decimal>>("SELECT COALESCE(SUM(grand_total), 0) FROM sales_invoices WHERE DATE(invoice_date) = CURRENT_DATE").fetch_one(&pool).await;
    println!("Q2: {:?}", q2);

    println!("Testing Q3...");
    let q3 = sqlx::query_scalar::<_, Option<Decimal>>("SELECT COALESCE(SUM(grand_total), 0) FROM sales_invoices WHERE DATE_TRUNC('month', invoice_date) = DATE_TRUNC('month', CURRENT_DATE)").fetch_one(&pool).await;
    println!("Q3: {:?}", q3);

    println!("Testing Q4...");
    let q4 = sqlx::query_scalar::<_, Option<Decimal>>("SELECT COALESCE(SUM(outstanding_balance), 0) FROM customers").fetch_one(&pool).await;
    println!("Q4: {:?}", q4);

    println!("Testing Q5...");
    let q5 = sqlx::query_scalar::<_, Option<Decimal>>("SELECT COALESCE(SUM(outstanding_balance), 0) FROM suppliers").fetch_one(&pool).await;
    println!("Q5: {:?}", q5);

    println!("Testing Q6 fixed...");
    let q6_fixed = sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*)::bigint FROM (
            SELECT p.id
            FROM products p
            LEFT JOIN inventory_balances ib ON ib.product_id = p.id
            WHERE p.status = 'active'
            GROUP BY p.id, p.reorder_threshold
            HAVING COALESCE(SUM(ib.available), 0) < p.reorder_threshold
        ) sub"#
    ).fetch_one(&pool).await;
    println!("Q6 fixed: {:?}", q6_fixed);

    println!("Testing sales_trend...");
    let trend = sqlx::query!(
        r#"SELECT DATE(invoice_date) as "date!", SUM(grand_total) as "total!"
           FROM sales_invoices
           WHERE invoice_date >= CURRENT_DATE - INTERVAL '30 days'
           GROUP BY DATE(invoice_date)
           ORDER BY "date!""#
    ).fetch_all(&pool).await;
    println!("trend: {:?}", trend);

    println!("Testing activity_feed...");
    let invoices = sqlx::query!(
        r#"SELECT 'invoice' as "type!", invoice_number as reference, grand_total as amount, created_at
           FROM sales_invoices ORDER BY created_at DESC LIMIT 10"#
    ).fetch_all(&pool).await;
    println!("invoices: {:?}", invoices);

    println!("Testing Q7...");
    let q7 = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sales_invoices WHERE due_date < CURRENT_DATE AND status IN ('issued', 'partially_paid')").fetch_one(&pool).await;
    println!("Q7: {:?}", q7);

    println!("Testing Q8...");
    let q8 = sqlx::query_scalar::<_, Option<Decimal>>("SELECT COALESCE(SUM(amount), 0) FROM payments WHERE DATE(payment_date) = CURRENT_DATE AND direction = 'inbound' AND status = 'verified'").fetch_one(&pool).await;
    println!("Q8: {:?}", q8);

    Ok(())
}
