use axum::{
    extract::State,
    http::{header, HeaderMap, HeaderName, HeaderValue, Method},
    routing::{delete, get, patch, post, put},
    Json, Router,
};
use serde_json::Value;
use tower_http::{
    compression::CompressionLayer,
    cors::{AllowOrigin, Any, CorsLayer},
    limit::RequestBodyLimitLayer,
    set_header::SetResponseHeaderLayer,
    trace::TraceLayer,
};

use crate::{
    config::AppConfig,
    database::DbPool,
    errors::{AppError, AppResult},
    middleware::auth_middleware::require_auth,
};

pub mod health;

use crate::audit::handlers as audit_handlers;
use crate::auth::handlers as auth_handlers;
use crate::cloudinary::handlers as cloudinary_handlers;
use crate::customers::handlers as customer_handlers;
use crate::finance::handlers as finance_handlers;
use crate::gst::handlers as gst_handlers;
use crate::inventory::handlers as inventory_handlers;
use crate::invoices::handlers as invoice_handlers;
use crate::notifications::handlers as notification_handlers;
use crate::payments::handlers as payment_handlers;
use crate::purchases::handlers as purchase_handlers;
use crate::racks::handlers as rack_handlers;
use crate::reports::handlers as report_handlers;
use crate::sales::handlers as sales_handlers;
use crate::settings as settings_handlers;
use crate::stock_movements::handlers as stock_handlers;
use crate::suppliers::handlers as supplier_handlers;
use crate::users::handlers as user_handlers;

#[derive(Clone)]
pub struct AppState {
    pub pool: DbPool,
    pub config: AppConfig,
}

fn cors_layer(config: &AppConfig) -> CorsLayer {
    let methods = [Method::GET, Method::POST, Method::PATCH, Method::PUT, Method::DELETE, Method::OPTIONS];
    let headers = [header::CONTENT_TYPE, header::AUTHORIZATION, header::ACCEPT];
    let origins = config.allowed_origins();

    if origins.is_empty() || origins.iter().any(|o| o == "*") {
        return CorsLayer::new().allow_origin(Any).allow_methods(methods).allow_headers(headers);
    }
    let parsed: Vec<HeaderValue> = origins.iter().filter_map(|o| o.parse().ok()).collect();
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(parsed))
        .allow_methods(methods)
        .allow_headers(headers)
        .allow_credentials(true)
}

/// HTTP trigger for scheduled jobs (e.g. Vercel Cron, which sends
/// `Authorization: Bearer $CRON_SECRET`). Disabled until CRON_SECRET is set.
async fn run_cron(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<Value>> {
    let secret = &state.config.cron_secret;
    if secret.is_empty() {
        return Err(AppError::NotFound("Not found".into()));
    }
    let provided = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or_default();

    // Compare digests so the check runs in constant time.
    use sha2::{Digest, Sha256};
    let a = Sha256::digest(secret.as_bytes());
    let b = Sha256::digest(provided.as_bytes());
    if a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) != 0 {
        return Err(AppError::Unauthorized("Invalid cron secret".into()));
    }

    Ok(Json(crate::jobs::run_all(&state.pool).await))
}

async fn not_found() -> AppError {
    AppError::NotFound("The requested endpoint does not exist".into())
}

pub fn create_router(pool: DbPool, config: AppConfig) -> Router {
    let cors = cors_layer(&config);
    let state = AppState { pool, config };

    let public_routes = Router::new()
        .route("/", get(|| async {
            Json(serde_json::json!({
                "service": "RARA Enterprise ERP Backend",
                "version": env!("CARGO_PKG_VERSION"),
                "status": "online"
            }))
        }))
        .route("/health", get(health::health_check))
        .route("/api/v1/health", get(health::health_check))
        .route("/api/v1/auth/login", post(auth_handlers::login))
        .route("/api/v1/auth/refresh", post(auth_handlers::refresh))
        .route("/api/v1/webhooks/razorpay", post(payment_handlers::razorpay_webhook))
        .route("/api/v1/cron/run", get(run_cron).post(run_cron));

    let protected_routes = Router::new()
        // Auth
        .route("/api/v1/auth/logout", post(auth_handlers::logout))
        .route("/api/v1/auth/me", get(auth_handlers::me))
        .route("/api/v1/auth/change-password", post(auth_handlers::change_password))

        // Users & RBAC
        .route("/api/v1/users", get(user_handlers::list_users).post(user_handlers::create_user))
        .route("/api/v1/users/:id", get(user_handlers::get_user).patch(user_handlers::update_user))
        .route("/api/v1/users/:id/deactivate", post(user_handlers::deactivate_user))
        .route("/api/v1/users/:id/reset-password", post(user_handlers::reset_password))
        .route("/api/v1/users/:id/permissions", get(user_handlers::get_permissions).put(user_handlers::update_permissions))
        .route("/api/v1/permissions", get(settings_handlers::list_permissions))

        // Settings
        .route("/api/v1/settings/company", get(settings_handlers::get_company).put(settings_handlers::update_company))

        // Dashboard
        .route("/api/v1/dashboard/summary", get(sales_handlers::dashboard_summary))
        .route("/api/v1/dashboard/sales", get(sales_handlers::sales_trend))
        .route("/api/v1/dashboard/activity", get(sales_handlers::activity_feed))
        .route("/api/v1/dashboard/top-products", get(sales_handlers::top_products))

        // Inventory
        .route("/api/v1/inventory/products", get(inventory_handlers::list_products).post(inventory_handlers::create_product))
        .route("/api/v1/inventory/products/:id", get(inventory_handlers::get_product).patch(inventory_handlers::update_product))
        .route("/api/v1/inventory/adjustments", post(inventory_handlers::adjust_stock))
        .route("/api/v1/inventory/categories", get(inventory_handlers::list_categories).post(inventory_handlers::create_category))
        .route("/api/v1/inventory/brands", get(inventory_handlers::list_brands).post(inventory_handlers::create_brand))
        .route("/api/v1/inventory/units", get(inventory_handlers::list_units).post(inventory_handlers::create_unit))
        .route("/api/v1/inventory/imports", get(inventory_handlers::list_imports).post(inventory_handlers::upload_import))
        .route("/api/v1/inventory/imports/:id", get(inventory_handlers::get_import))
        .route("/api/v1/inventory/movements", get(stock_handlers::list_movements))
        .route("/api/v1/inventory/stock-counts", post(stock_handlers::create_stock_count))

        // Warehouses & racks
        .route("/api/v1/warehouses", get(rack_handlers::list_warehouses).post(rack_handlers::create_warehouse))
        .route("/api/v1/racks", get(rack_handlers::list_racks).post(rack_handlers::create_rack))
        .route("/api/v1/racks/:id/inventory", get(rack_handlers::rack_inventory))
        .route("/api/v1/racks/transfers", post(rack_handlers::rack_transfer))

        // Purchases
        .route("/api/v1/purchases", get(purchase_handlers::list_purchases).post(purchase_handlers::create_purchase))
        .route("/api/v1/purchases/:id", get(purchase_handlers::get_purchase).patch(purchase_handlers::update_purchase))
        .route("/api/v1/purchases/:id/approve", post(purchase_handlers::approve_purchase))
        .route("/api/v1/purchases/:id/cancel", post(purchase_handlers::cancel_purchase))
        .route("/api/v1/purchases/:id/receipts", post(purchase_handlers::receive_goods))
        .route("/api/v1/purchases/:id/payments", post(purchase_handlers::record_supplier_payment))

        // Suppliers
        .route("/api/v1/suppliers", get(supplier_handlers::list_suppliers).post(supplier_handlers::create_supplier))
        .route("/api/v1/suppliers/:id", get(supplier_handlers::get_supplier).patch(supplier_handlers::update_supplier))
        .route("/api/v1/suppliers/:id/ledger", get(supplier_handlers::supplier_ledger))

        // Customers
        .route("/api/v1/customers", get(customer_handlers::list_customers).post(customer_handlers::create_customer))
        .route("/api/v1/customers/:id", get(customer_handlers::get_customer).patch(customer_handlers::update_customer))
        .route("/api/v1/customers/:id/ledger", get(customer_handlers::customer_ledger))

        // Sales / invoices
        .route("/api/v1/sales/invoices", post(invoice_handlers::create_invoice))
        .route("/api/v1/invoices", get(invoice_handlers::list_invoices).post(invoice_handlers::create_invoice))
        .route("/api/v1/invoices/:id", get(invoice_handlers::get_invoice))
        .route("/api/v1/invoices/:id/cancel", post(invoice_handlers::cancel_invoice))
        .route("/api/v1/invoices/:id/payments", post(invoice_handlers::record_payment))
        .route("/api/v1/invoices/:id/credit-notes", post(invoice_handlers::create_credit_note))
        .route("/api/v1/invoices/:id/upi-details", get(payment_handlers::upi_payment_details))
        .route("/api/v1/invoices/:id/pdf", get(invoice_handlers::download_pdf))

        // Payments
        .route("/api/v1/payments", get(payment_handlers::list_payments))
        .route("/api/v1/payments/:id", get(payment_handlers::get_payment))
        .route("/api/v1/payments/razorpay/orders", post(payment_handlers::create_razorpay_order))

        // Finance
        .route("/api/v1/finance/payables", get(finance_handlers::list_payables))
        .route("/api/v1/finance/receivables", get(finance_handlers::list_receivables))
        .route("/api/v1/finance/expenses", get(finance_handlers::list_expenses).post(finance_handlers::create_expense))
        .route("/api/v1/finance/expenses/:id", put(finance_handlers::update_expense).delete(finance_handlers::delete_expense))
        .route("/api/v1/finance/due-date/:kind/:id", put(finance_handlers::update_due_date))
        .route("/api/v1/finance/expenses/:id/:decision", post(finance_handlers::review_expense))
        .route("/api/v1/finance/summary", get(finance_handlers::financial_summary))

        // GST verification
        .route("/api/v1/gst/verify", post(gst_handlers::verify_gstin))
        .route("/api/v1/gst/verifications", get(gst_handlers::list_verifications))
        .route("/api/v1/gst/verifications/:id", get(gst_handlers::get_verification))

        // Image uploads
        .route("/api/v1/uploads/images", post(cloudinary_handlers::upload_image))
        .route("/api/v1/uploads/images/:id", delete(cloudinary_handlers::delete_image))

        // Notifications
        .route("/api/v1/notifications", get(notification_handlers::list_notifications))
        .route("/api/v1/notifications/:id/read", patch(notification_handlers::mark_read))
        .route("/api/v1/notifications/read-all", post(notification_handlers::mark_all_read))

        // Reports
        .route("/api/v1/reports/inventory", get(report_handlers::inventory_report))
        .route("/api/v1/reports/sales", get(report_handlers::sales_report))
        .route("/api/v1/reports/purchases", get(report_handlers::purchases_report))
        .route("/api/v1/reports/payables", get(report_handlers::payables_report))
        .route("/api/v1/reports/receivables", get(report_handlers::receivables_report))
        .route("/api/v1/reports/gst", get(report_handlers::gst_report))

        // Audit logs
        .route("/api/v1/audit-logs", get(audit_handlers::list_audit_logs))

        .layer(axum::middleware::from_fn_with_state(state.clone(), require_auth));

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .fallback(not_found)
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("x-content-type-options"),
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("x-frame-options"),
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(header::CACHE_CONTROL, HeaderValue::from_static("no-store")))
        .layer(CompressionLayer::new())
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .layer(axum::extract::DefaultBodyLimit::max(25 * 1024 * 1024))
        .layer(RequestBodyLimitLayer::new(25 * 1024 * 1024))
        .with_state(state)
}
