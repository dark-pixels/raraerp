//! Bulk product / stock import from CSV or Excel.
//!
//! Columns are matched by header name (case-insensitive), so column order does
//! not matter. Recognised headers:
//!   sku*, name, category, brand, unit, barcode, hsn_code, gst_rate,
//!   purchase_price, retail_price, wholesale_price, reorder_threshold,
//!   quantity, rack_code
//! Each row is applied in its own savepoint, so one bad row never aborts the file.

use std::collections::HashMap;
use std::io::Cursor;

use calamine::{open_workbook_from_rs, Data, Reader, Xlsx};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::database::DbPool;
use crate::errors::{AppError, AppResult};
use crate::inventory::stock;

pub struct ImportSummary {
    pub total: i32,
    pub success: i32,
    pub errors: i32,
    /// First few error messages, for immediate display.
    pub messages: Vec<String>,
}

/// Splits CSV text into records, honouring quoted fields with embedded commas,
/// quotes ("") and newlines.
pub fn parse_csv(content: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = content.trim_start_matches('\u{feff}').chars().peekable();

    while let Some(c) = chars.next() {
        if in_quotes {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    field.push('"');
                    chars.next();
                }
                '"' => in_quotes = false,
                _ => field.push(c),
            }
        } else {
            match c {
                '"' => in_quotes = true,
                ',' => row.push(std::mem::take(&mut field)),
                '\r' => {}
                '\n' => {
                    row.push(std::mem::take(&mut field));
                    rows.push(std::mem::take(&mut row));
                }
                _ => field.push(c),
            }
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows.retain(|r| r.iter().any(|f| !f.trim().is_empty()));
    rows
}

fn parse_xlsx(data: &[u8]) -> AppResult<Vec<Vec<String>>> {
    let mut workbook: Xlsx<_> = open_workbook_from_rs(Cursor::new(data.to_vec()))
        .map_err(|e| AppError::Validation(format!("Could not read Excel file: {}", e)))?;
    let range = workbook
        .worksheet_range_at(0)
        .ok_or_else(|| AppError::Validation("The Excel file has no worksheets".into()))?
        .map_err(|e| AppError::Validation(format!("Could not read worksheet: {}", e)))?;

    Ok(range
        .rows()
        .map(|r| {
            r.iter()
                .map(|cell| match cell {
                    Data::Empty => String::new(),
                    // Avoid "12.0" for whole numbers such as SKUs or quantities.
                    Data::Float(f) if f.fract() == 0.0 && f.abs() < 1e15 => format!("{}", *f as i64),
                    other => other.to_string(),
                })
                .collect::<Vec<_>>()
        })
        .filter(|r| r.iter().any(|f| !f.trim().is_empty()))
        .collect())
}

struct Row<'a> {
    cols: &'a HashMap<String, usize>,
    values: &'a [String],
}

impl Row<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.cols
            .get(key)
            .and_then(|&i| self.values.get(i))
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
    }
    fn dec(&self, key: &str) -> Result<Option<Decimal>, String> {
        match self.get(key) {
            None => Ok(None),
            Some(v) => v
                .replace([',', '₹'], "")
                .trim()
                .parse::<Decimal>()
                .map(Some)
                .map_err(|_| format!("{} '{}' is not a number", key, v)),
        }
    }
}

fn normalize_header(h: &str) -> String {
    let h = h.trim().to_lowercase().replace([' ', '-'], "_");
    match h.as_str() {
        "product_name" | "item" | "item_name" | "product" => "name".into(),
        "hsn" => "hsn_code".into(),
        "gst" | "gst_percent" | "tax_rate" => "gst_rate".into(),
        "cost" | "cost_price" => "purchase_price".into(),
        "mrp" | "price" | "selling_price" => "retail_price".into(),
        "qty" | "stock" | "opening_stock" => "quantity".into(),
        "rack" | "location" => "rack_code".into(),
        "min_stock" | "reorder_level" => "reorder_threshold".into(),
        _ => h,
    }
}

async fn lookup_or_create_category(tx: &mut stock::Tx<'_>, name: &str) -> AppResult<Uuid> {
    if let Some(id) = sqlx::query_scalar!(
        "SELECT id FROM categories WHERE LOWER(name) = LOWER($1) OR slug = LOWER($1) LIMIT 1",
        name
    )
    .fetch_optional(&mut **tx)
    .await?
    {
        return Ok(id);
    }
    let slug: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    Ok(sqlx::query_scalar!(
        "INSERT INTO categories (name, slug) VALUES ($1, $2) ON CONFLICT (slug) DO UPDATE SET name = categories.name RETURNING id",
        name, slug
    )
    .fetch_one(&mut **tx)
    .await?)
}

async fn lookup_or_create_brand(tx: &mut stock::Tx<'_>, name: &str) -> AppResult<Uuid> {
    if let Some(id) = sqlx::query_scalar!("SELECT id FROM brands WHERE LOWER(name) = LOWER($1)", name)
        .fetch_optional(&mut **tx)
        .await?
    {
        return Ok(id);
    }
    Ok(sqlx::query_scalar!(
        "INSERT INTO brands (name) VALUES ($1) ON CONFLICT (name) DO UPDATE SET name = EXCLUDED.name RETURNING id",
        name
    )
    .fetch_one(&mut **tx)
    .await?)
}

async fn lookup_unit(tx: &mut stock::Tx<'_>, name: Option<&str>) -> AppResult<Option<Uuid>> {
    let found = match name {
        Some(n) => sqlx::query_scalar!(
            "SELECT id FROM units_of_measure WHERE LOWER(name) = LOWER($1) OR LOWER(abbreviation) = LOWER($1) LIMIT 1",
            n
        )
        .fetch_optional(&mut **tx)
        .await?,
        None => None,
    };
    match found {
        Some(id) => Ok(Some(id)),
        None => Ok(sqlx::query_scalar!("SELECT id FROM units_of_measure ORDER BY (abbreviation = 'Pcs') DESC LIMIT 1")
            .fetch_optional(&mut **tx)
            .await?),
    }
}

async fn location_for(tx: &mut stock::Tx<'_>, rack_code: Option<&str>, fallback: Option<Uuid>) -> AppResult<Uuid> {
    if let Some(code) = rack_code {
        if let Some(id) = sqlx::query_scalar!(
            "SELECT id FROM rack_locations WHERE LOWER(rack_code) = LOWER($1) AND is_active ORDER BY created_at LIMIT 1",
            code
        )
        .fetch_optional(&mut **tx)
        .await?
        {
            return Ok(id);
        }
        return Err(AppError::Validation(format!("Rack '{}' does not exist", code)));
    }
    stock::resolve_location(tx, fallback).await
}

async fn apply_row(
    tx: &mut stock::Tx<'_>,
    row: &Row<'_>,
    mode: &str,
    user_id: Uuid,
    import_id: Uuid,
    default_location: Option<Uuid>,
) -> Result<(), String> {
    let sku = row.get("sku").map(|s| s.to_uppercase()).ok_or("SKU is required")?;
    let existing: Option<Uuid> = sqlx::query_scalar!("SELECT id FROM products WHERE UPPER(sku) = $1", sku)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|e| e.to_string())?;

    let purchase = row.dec("purchase_price")?;
    let retail = row.dec("retail_price")?;
    let wholesale = row.dec("wholesale_price")?;
    let gst = row.dec("gst_rate")?;
    if let Some(g) = gst {
        if g < Decimal::ZERO || g > Decimal::from(100) {
            return Err("gst_rate must be between 0 and 100".into());
        }
    }
    let reorder = row.dec("reorder_threshold")?.and_then(|d| d.trunc().to_string().parse::<i32>().ok());
    let err = |e: AppError| e.to_string().trim_start_matches("Validation error: ").to_string();

    match mode {
        "create" => {
            if existing.is_some() {
                return Err(format!("SKU {} already exists (use Update mode)", sku));
            }
            let name = row.get("name").ok_or("name is required")?;
            let category = lookup_or_create_category(tx, row.get("category").unwrap_or("Other Hardware")).await.map_err(err)?;
            let brand = match row.get("brand") {
                Some(b) => Some(lookup_or_create_brand(tx, b).await.map_err(err)?),
                None => None,
            };
            let unit = lookup_unit(tx, row.get("unit")).await.map_err(err)?.ok_or("No units of measure are configured")?;
            let id = sqlx::query_scalar!(
                r#"INSERT INTO products (sku, barcode, name, category_id, brand_id, unit_id, hsn_code,
                       purchase_price, retail_price, wholesale_price, gst_rate, reorder_threshold, created_by)
                   VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13) RETURNING id"#,
                sku, row.get("barcode"), name, category, brand, unit, row.get("hsn_code"),
                purchase.unwrap_or(Decimal::ZERO), retail.unwrap_or(Decimal::ZERO),
                wholesale.or(retail).unwrap_or(Decimal::ZERO), gst.unwrap_or(Decimal::from(18)),
                reorder.unwrap_or(10), user_id
            )
            .fetch_one(&mut **tx)
            .await
            .map_err(|e| crate::errors::AppError::Database(e).to_string())?;

            if let Some(qty) = row.dec("quantity")?.filter(|q| *q > Decimal::ZERO) {
                let loc = location_for(tx, row.get("rack_code"), default_location).await.map_err(err)?;
                stock::add_stock(tx, id, loc, qty, "opening_stock", Some(&import_id.to_string()), user_id, Some("Bulk import")).await.map_err(err)?;
            }
        }
        "update" => {
            let id = existing.ok_or_else(|| format!("SKU {} not found", sku))?;
            let category = match row.get("category") {
                Some(c) => Some(lookup_or_create_category(tx, c).await.map_err(err)?),
                None => None,
            };
            let brand = match row.get("brand") {
                Some(b) => Some(lookup_or_create_brand(tx, b).await.map_err(err)?),
                None => None,
            };
            sqlx::query!(
                r#"UPDATE products SET
                       name = COALESCE($1, name), barcode = COALESCE($2, barcode),
                       category_id = COALESCE($3, category_id), brand_id = COALESCE($4, brand_id),
                       hsn_code = COALESCE($5, hsn_code), purchase_price = COALESCE($6, purchase_price),
                       retail_price = COALESCE($7, retail_price), wholesale_price = COALESCE($8, wholesale_price),
                       gst_rate = COALESCE($9, gst_rate), reorder_threshold = COALESCE($10, reorder_threshold),
                       updated_at = NOW()
                   WHERE id = $11"#,
                row.get("name"), row.get("barcode"), category, brand, row.get("hsn_code"),
                purchase, retail, wholesale, gst, reorder, id
            )
            .execute(&mut **tx)
            .await
            .map_err(|e| crate::errors::AppError::Database(e).to_string())?;
        }
        "opening_stock" | "adjustment" => {
            let id = existing.ok_or_else(|| format!("SKU {} not found", sku))?;
            let qty = row.dec("quantity")?.ok_or("quantity is required")?;
            let loc = location_for(tx, row.get("rack_code"), default_location).await.map_err(err)?;
            let reference = import_id.to_string();
            if mode == "opening_stock" {
                if qty < Decimal::ZERO {
                    return Err("Opening stock cannot be negative".into());
                }
                stock::add_stock(tx, id, loc, qty, "opening_stock", Some(&reference), user_id, Some("Bulk import")).await.map_err(err)?;
            } else if qty > Decimal::ZERO {
                stock::add_stock(tx, id, loc, qty, "approved_adjustment", Some(&reference), user_id, Some("Bulk adjustment")).await.map_err(err)?;
            } else if qty < Decimal::ZERO {
                stock::remove_stock_at(tx, id, loc, -qty, "approved_adjustment", Some(&reference), user_id, Some("Bulk adjustment")).await.map_err(err)?;
            }
        }
        _ => return Err("Unknown import mode".into()),
    }
    Ok(())
}

pub async fn process_import(
    pool: &DbPool,
    import_id: Uuid,
    filename: &str,
    file_data: &[u8],
    user_id: Uuid,
    mode: &str,
    default_location: Option<Uuid>,
) -> AppResult<ImportSummary> {
    let lower = filename.to_lowercase();
    let records = if lower.ends_with(".xlsx") || lower.ends_with(".xlsm") {
        parse_xlsx(file_data)?
    } else if lower.ends_with(".xls") {
        return Err(AppError::Validation("Legacy .xls files are not supported. Save as .xlsx or .csv".into()));
    } else {
        parse_csv(&String::from_utf8_lossy(file_data))
    };

    let mut iter = records.into_iter();
    let header = iter.next().ok_or_else(|| AppError::Validation("The file is empty".into()))?;
    let cols: HashMap<String, usize> = header.iter().enumerate().map(|(i, h)| (normalize_header(h), i)).collect();
    if !cols.contains_key("sku") {
        return Err(AppError::Validation("The first row must be a header row containing a 'sku' column".into()));
    }
    if matches!(mode, "opening_stock" | "adjustment") && !cols.contains_key("quantity") {
        return Err(AppError::Validation("Stock imports need a 'quantity' column".into()));
    }

    let mut summary = ImportSummary { total: 0, success: 0, errors: 0, messages: Vec::new() };
    let mut tx = pool.begin().await?;

    for (i, values) in iter.enumerate() {
        if summary.total >= 10_000 {
            break;
        }
        let row_number = i as i32 + 2; // 1-based, after the header
        summary.total += 1;
        let row = Row { cols: &cols, values: &values };

        sqlx::query("SAVEPOINT import_row").execute(&mut *tx).await?;
        match apply_row(&mut tx, &row, mode, user_id, import_id, default_location).await {
            Ok(()) => {
                sqlx::query("RELEASE SAVEPOINT import_row").execute(&mut *tx).await?;
                summary.success += 1;
            }
            Err(msg) => {
                sqlx::query("ROLLBACK TO SAVEPOINT import_row").execute(&mut *tx).await?;
                summary.errors += 1;
                if summary.messages.len() < 20 {
                    summary.messages.push(format!("Row {}: {}", row_number, msg));
                }
                sqlx::query!(
                    "INSERT INTO inventory_import_errors (import_id, row_number, error_message, raw_data) VALUES ($1, $2, $3, $4)",
                    import_id, row_number, msg, values.join(",")
                )
                .execute(&mut *tx)
                .await?;
            }
        }
    }

    let status = if summary.success == 0 && summary.total > 0 { "failed" } else { "completed" };
    sqlx::query!(
        r#"UPDATE inventory_imports
           SET status = $1, total_rows = $2, success_count = $3, error_count = $4, completed_at = NOW()
           WHERE id = $5"#,
        status, summary.total, summary.success, summary.errors, import_id
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_handles_quotes_and_crlf() {
        let rows = parse_csv("sku,name,price\r\nA1,\"Hinge, soft close\",120\r\n\r\nB2,\"6\"\" handle\",80\n");
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[1], vec!["A1", "Hinge, soft close", "120"]);
        assert_eq!(rows[2], vec!["B2", "6\" handle", "80"]);
    }

    #[test]
    fn csv_strips_bom_and_handles_no_trailing_newline() {
        let rows = parse_csv("\u{feff}sku,qty\nX,5");
        assert_eq!(rows, vec![vec!["sku", "qty"], vec!["X", "5"]]);
    }

    #[test]
    fn header_aliases() {
        assert_eq!(normalize_header("Product Name"), "name");
        assert_eq!(normalize_header("MRP"), "retail_price");
        assert_eq!(normalize_header("Opening Stock"), "quantity");
        assert_eq!(normalize_header("SKU"), "sku");
    }
}
