use crate::database::DbPool;
use crate::errors::AppResult;
use uuid::Uuid;
use calamine::{Reader, open_workbook_from_rs, Xlsx, RangeDeserializerBuilder};
use std::io::Cursor;

pub async fn process_import(
    pool: DbPool,
    import_id: Uuid,
    file_data: Vec<u8>,
    user_id: Uuid,
    mode: &str,
) -> AppResult<()> {
    let _ = sqlx::query!(
        "UPDATE inventory_imports SET status = 'processing' WHERE id = $1",
        import_id
    )
    .execute(&pool)
    .await;

    let mut success = 0i32;
    let mut errors = 0i32;

    // Try to parse as CSV (simple line-by-line)
    let content = String::from_utf8_lossy(&file_data);
    let mut lines = content.lines();
    
    // Skip header
    let _ = lines.next();
    let mut total = 0i32;

    for line in lines {
        total += 1;
        let cols: Vec<&str> = line.split(',').collect();
        if cols.len() < 3 {
            errors += 1;
            sqlx::query!(
                "INSERT INTO inventory_import_errors (import_id, row_number, error_message, raw_data)
                 VALUES ($1, $2, $3, $4)",
                import_id, total, "Insufficient columns", line
            ).execute(&pool).await.ok();
            continue;
        }

        let sku = cols[0].trim();
        let name = cols[1].trim();

        if sku.is_empty() || name.is_empty() {
            errors += 1;
            continue;
        }

        // Check if product exists
        let existing = sqlx::query_scalar!("SELECT id FROM products WHERE sku = $1", sku)
            .fetch_optional(&pool)
            .await
            .unwrap_or(None);

        match mode {
            "create" | "update" => {
                if existing.is_none() && mode == "create" {
                    let cat_id = sqlx::query_scalar!("SELECT id FROM categories LIMIT 1")
                        .fetch_optional(&pool).await.unwrap_or(None);
                    let unit_id = sqlx::query_scalar!("SELECT id FROM units_of_measure LIMIT 1")
                        .fetch_optional(&pool).await.unwrap_or(None);

                    if let (Some(cat), Some(unit)) = (cat_id, unit_id) {
                        let result = sqlx::query!(
                            r#"INSERT INTO products (sku, name, category_id, unit_id, purchase_price, retail_price, wholesale_price, gst_rate)
                               VALUES ($1, $2, $3, $4, 0, 0, 0, 18) ON CONFLICT (sku) DO NOTHING"#,
                            sku, name, cat, unit
                        ).execute(&pool).await;
                        if result.is_ok() { success += 1; } else { errors += 1; }
                    } else {
                        errors += 1;
                    }
                } else if existing.is_some() && mode == "update" {
                    sqlx::query!("UPDATE products SET name = $1, updated_at = NOW() WHERE sku = $2", name, sku)
                        .execute(&pool).await.ok();
                    success += 1;
                } else {
                    errors += 1;
                }
            }
            _ => { errors += 1; }
        }
    }

    sqlx::query!(
        r#"UPDATE inventory_imports
           SET status = 'completed', total_rows = $1, success_count = $2, error_count = $3, completed_at = NOW()
           WHERE id = $4"#,
        total, success, errors, import_id
    )
    .execute(&pool)
    .await?;

    Ok(())
}
