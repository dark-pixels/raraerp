//! Shared request-parsing helpers.
//!
//! The frontend sends numeric fields sometimes as JSON numbers and sometimes as
//! strings (form inputs). Every handler goes through these helpers so both
//! shapes are accepted consistently instead of silently defaulting to zero.

use rust_decimal::{prelude::FromPrimitive, Decimal, RoundingStrategy};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::errors::{AppError, AppResult};

/// Parses a decimal from a JSON number or numeric string. Empty strings are `None`.
pub fn dec(v: Option<&Value>) -> Option<Decimal> {
    match v? {
        Value::String(s) => {
            let s = s.trim().replace(',', "");
            if s.is_empty() { None } else { s.parse::<Decimal>().ok() }
        }
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(Decimal::from(i))
            } else {
                n.as_f64().and_then(Decimal::from_f64)
            }
        }
        _ => None,
    }
}

/// Like [`dec`] but returns a validation error naming the field when it is
/// present yet not a valid number.
pub fn dec_field(body: &Value, field: &str) -> AppResult<Option<Decimal>> {
    match body.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.trim().is_empty() => Ok(None),
        Some(v) => dec(Some(v))
            .map(Some)
            .ok_or_else(|| AppError::Validation(format!("{} must be a number", field))),
    }
}

/// Parses an integer from a JSON number or numeric string.
pub fn int(v: Option<&Value>) -> Option<i64> {
    match v? {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        Value::String(s) => s.trim().parse::<i64>().ok(),
        _ => None,
    }
}

/// Parses a UUID from a JSON string.
pub fn uuid(v: Option<&Value>) -> Option<Uuid> {
    v?.as_str().and_then(|s| s.trim().parse().ok())
}

/// Returns a trimmed, non-empty string.
pub fn text(v: Option<&Value>) -> Option<String> {
    v?.as_str().map(str::trim).filter(|s| !s.is_empty()).map(String::from)
}

/// Returns a required trimmed, non-empty string or a validation error.
pub fn required_text(body: &Value, field: &str) -> AppResult<String> {
    text(body.get(field)).ok_or_else(|| AppError::Validation(format!("{} is required", field)))
}

pub fn bool_field(v: Option<&Value>) -> Option<bool> {
    match v? {
        Value::Bool(b) => Some(*b),
        Value::String(s) => match s.as_str() {
            "true" | "1" => Some(true),
            "false" | "0" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

/// Rounds a monetary amount to 2 decimal places (half away from zero).
pub fn money(d: Decimal) -> Decimal {
    d.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}

pub const PAYMENT_METHODS: &[&str] = &["cash", "bank_transfer", "razorpay", "upi", "cheque"];

pub fn payment_method(v: Option<&Value>) -> AppResult<String> {
    let method = text(v).unwrap_or_else(|| "cash".to_string()).to_lowercase();
    if PAYMENT_METHODS.contains(&method.as_str()) {
        Ok(method)
    } else {
        Err(AppError::Validation(format!(
            "payment_method must be one of: {}",
            PAYMENT_METHODS.join(", ")
        )))
    }
}

/// Validates a GSTIN's shape (15 chars: 2-digit state code, PAN, entity, 'Z', checksum).
pub fn normalize_gstin(v: Option<&Value>) -> AppResult<Option<String>> {
    let Some(g) = text(v) else { return Ok(None) };
    let g = g.to_uppercase();
    let valid = g.len() == 15
        && g.chars().all(|c| c.is_ascii_alphanumeric())
        && g[..2].chars().all(|c| c.is_ascii_digit());
    if valid {
        Ok(Some(g))
    } else {
        Err(AppError::Validation("GSTIN must be 15 characters and start with a 2-digit state code".into()))
    }
}

/// Deserializes a query value, treating empty or unparsable input as absent
/// rather than rejecting the whole request.
fn lenient<'de, D, T>(de: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: std::str::FromStr,
{
    let raw: Option<String> = Option::deserialize(de)?;
    Ok(raw.as_deref().map(str::trim).filter(|s| !s.is_empty()).and_then(|s| s.parse().ok()))
}

/// Common list query parameters shared by all list endpoints. Unknown
/// parameters are ignored and malformed values are treated as absent.
#[derive(Debug, Default, Deserialize)]
pub struct ListQuery {
    #[serde(default, deserialize_with = "lenient")]
    pub page: Option<i64>,
    #[serde(default, deserialize_with = "lenient")]
    pub per_page: Option<i64>,
    #[serde(default, deserialize_with = "lenient")]
    pub limit: Option<i64>,
    pub search: Option<String>,
    pub status: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub from: Option<chrono::NaiveDate>,
    #[serde(default, deserialize_with = "lenient")]
    pub to: Option<chrono::NaiveDate>,
    #[serde(default, deserialize_with = "lenient")]
    pub customer_id: Option<Uuid>,
    #[serde(default, deserialize_with = "lenient")]
    pub supplier_id: Option<Uuid>,
    #[serde(default, deserialize_with = "lenient")]
    pub product_id: Option<Uuid>,
    #[serde(default, deserialize_with = "lenient")]
    pub category_id: Option<Uuid>,
    #[serde(default, deserialize_with = "lenient")]
    pub brand_id: Option<Uuid>,
    #[serde(default, deserialize_with = "lenient")]
    pub warehouse_id: Option<Uuid>,
    #[serde(default, deserialize_with = "lenient")]
    pub low_stock: Option<bool>,
    pub direction: Option<String>,
    pub payment_status: Option<String>,
    pub method: Option<String>,
    pub role: Option<String>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
}

impl ListQuery {
    pub fn per_page(&self) -> i64 {
        self.per_page.or(self.limit).unwrap_or(50).clamp(1, 500)
    }
    pub fn page(&self) -> i64 {
        self.page.unwrap_or(1).max(1)
    }
    pub fn offset(&self) -> i64 {
        (self.page() - 1) * self.per_page()
    }
    pub fn search(&self) -> Option<String> {
        non_empty(&self.search)
    }
    pub fn status(&self) -> Option<String> {
        non_empty(&self.status)
    }
}

pub fn non_empty(v: &Option<String>) -> Option<String> {
    v.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(String::from)
}

/// Standard paginated response envelope.
pub fn paginated(data: Vec<Value>, total: i64, q: &ListQuery) -> Value {
    let per_page = q.per_page();
    serde_json::json!({
        "data": data,
        "total": total,
        "page": q.page(),
        "per_page": per_page,
        "total_pages": (total + per_page - 1) / per_page,
    })
}

/// Formats an amount with Indian digit grouping: 12,34,567.89
pub fn format_inr(d: Decimal) -> String {
    let d = d.round_dp(2);
    let negative = d.is_sign_negative() && !d.is_zero();
    let s = format!("{:.2}", d.abs());
    let (int_part, frac) = s.split_once('.').unwrap_or((&s, "00"));
    let len = int_part.len();
    let mut out = String::with_capacity(len + len / 2);
    for (i, ch) in int_part.chars().enumerate() {
        out.push(ch);
        let remaining = len - i - 1;
        // Comma before the last 3 digits, then every 2 digits (lakh/crore grouping).
        if remaining == 3 || (remaining > 3 && (remaining - 3) % 2 == 0) {
            out.push(',');
        }
    }
    format!("{}{}.{}", if negative { "-" } else { "" }, out, frac)
}

/// Business "today" in Indian Standard Time. Servers run in UTC, so using
/// CURRENT_DATE would date late-evening sales to the previous day.
pub fn today() -> chrono::NaiveDate {
    (chrono::Utc::now() + chrono::Duration::minutes(330)).date_naive()
}

/// Allocates the next sequential document number, e.g. `INV-2026-000042`.
/// A transaction-scoped advisory lock serialises concurrent allocations so
/// two simultaneous sales can never receive the same number.
pub async fn next_document_number(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    table: &'static str,
    column: &'static str,
    prefix: &str,
) -> AppResult<String> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(format!("{}.{}", table, column))
        .execute(&mut **tx)
        .await?;

    let sql = format!(
        r"SELECT MAX(substring({column} from '(\d+)$')::bigint) FROM {table} WHERE {column} LIKE $1"
    );
    let max: Option<i64> = sqlx::query_scalar(&sql)
        .bind(format!("{}-%", prefix))
        .fetch_one(&mut **tx)
        .await?;
    Ok(format!("{}-{:06}", prefix, max.unwrap_or(0) + 1))
}

/// Best-effort client IP from proxy headers.
pub fn client_ip(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .or_else(|| headers.get("x-real-ip").and_then(|v| v.to_str().ok()))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn dec_accepts_numbers_and_strings() {
        assert_eq!(dec(Some(&json!(12))), Some(Decimal::from(12)));
        assert_eq!(dec(Some(&json!("12.50"))), Some("12.50".parse().unwrap()));
        assert_eq!(dec(Some(&json!(" 1,250.5 "))), Some("1250.5".parse().unwrap()));
        assert_eq!(dec(Some(&json!(""))), None);
        assert_eq!(dec(Some(&json!(null))), None);
        assert_eq!(dec(Some(&json!(2.5))).map(money), Some("2.50".parse().unwrap()));
    }

    #[test]
    fn dec_field_rejects_garbage() {
        let body = json!({"a": "abc", "b": "", "c": 3});
        assert!(dec_field(&body, "a").is_err());
        assert_eq!(dec_field(&body, "b").unwrap(), None);
        assert_eq!(dec_field(&body, "c").unwrap(), Some(Decimal::from(3)));
        assert_eq!(dec_field(&body, "missing").unwrap(), None);
    }

    #[test]
    fn money_rounds_half_away_from_zero() {
        assert_eq!(money("10.005".parse().unwrap()), "10.01".parse::<Decimal>().unwrap());
        assert_eq!(money("10.004".parse().unwrap()), "10.00".parse::<Decimal>().unwrap());
    }

    #[test]
    fn gstin_validation() {
        assert_eq!(
            normalize_gstin(Some(&json!("27aapfu0939f1zv"))).unwrap(),
            Some("27AAPFU0939F1ZV".to_string())
        );
        assert!(normalize_gstin(Some(&json!("ABC"))).is_err());
        assert_eq!(normalize_gstin(Some(&json!(""))).unwrap(), None);
    }

    #[test]
    fn list_query_bounds() {
        let q = ListQuery { per_page: Some(10_000), page: Some(0), ..Default::default() };
        assert_eq!(q.per_page(), 500);
        assert_eq!(q.page(), 1);
        assert_eq!(q.offset(), 0);
    }
}
