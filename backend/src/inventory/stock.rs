//! Stock ledger primitives. Every change to `inventory_balances` goes through
//! here so the balance and the `stock_movements` audit trail never diverge.

use rust_decimal::Decimal;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::errors::{AppError, AppResult};

pub type Tx<'a> = Transaction<'a, Postgres>;

/// Returns the default rack location, creating "MAIN / GENERAL" if the shop has
/// not configured any racks yet, so stock can always be received.
pub async fn default_location(tx: &mut Tx<'_>) -> AppResult<Uuid> {
    if let Some(id) = sqlx::query_scalar!(
        r#"SELECT rl.id FROM rack_locations rl JOIN warehouses w ON w.id = rl.warehouse_id
           WHERE rl.is_active AND w.is_active
           ORDER BY (rl.rack_code = 'GENERAL') DESC, w.created_at, rl.created_at LIMIT 1"#
    )
    .fetch_optional(&mut **tx)
    .await?
    {
        return Ok(id);
    }

    let warehouse_id = match sqlx::query_scalar!("SELECT id FROM warehouses WHERE is_active ORDER BY created_at LIMIT 1")
        .fetch_optional(&mut **tx)
        .await?
    {
        Some(id) => id,
        None => sqlx::query_scalar!(
            "INSERT INTO warehouses (name, code) VALUES ('Main Shop', 'MAIN') ON CONFLICT (code) DO UPDATE SET is_active = true RETURNING id"
        )
        .fetch_one(&mut **tx)
        .await?,
    };

    let id = sqlx::query_scalar!(
        "INSERT INTO rack_locations (warehouse_id, rack_code) VALUES ($1, 'GENERAL') RETURNING id",
        warehouse_id
    )
    .fetch_one(&mut **tx)
    .await?;
    Ok(id)
}

/// Resolves an optional explicit location, verifying it exists.
pub async fn resolve_location(tx: &mut Tx<'_>, location_id: Option<Uuid>) -> AppResult<Uuid> {
    match location_id {
        Some(id) => {
            let exists: bool = sqlx::query_scalar!(
                r#"SELECT EXISTS(SELECT 1 FROM rack_locations WHERE id = $1) AS "e!""#,
                id
            )
            .fetch_one(&mut **tx)
            .await?;
            if exists { Ok(id) } else { Err(AppError::Validation("Rack location not found".into())) }
        }
        None => default_location(tx).await,
    }
}

/// Adds stock to a location and records the movement.
#[allow(clippy::too_many_arguments)]
pub async fn add_stock(
    tx: &mut Tx<'_>,
    product_id: Uuid,
    location_id: Uuid,
    qty: Decimal,
    movement_type: &str,
    reference: Option<&str>,
    user_id: Uuid,
    reason: Option<&str>,
) -> AppResult<()> {
    if qty <= Decimal::ZERO {
        return Ok(());
    }
    sqlx::query!(
        r#"INSERT INTO inventory_balances (product_id, location_id, on_hand)
           VALUES ($1, $2, $3)
           ON CONFLICT (product_id, location_id) DO UPDATE
           SET on_hand = inventory_balances.on_hand + EXCLUDED.on_hand, updated_at = NOW()"#,
        product_id, location_id, qty
    )
    .execute(&mut **tx)
    .await?;

    sqlx::query!(
        r#"INSERT INTO stock_movements (product_id, quantity, destination_location_id, movement_type, reference_document, user_id, reason)
           VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
        product_id, qty, location_id, movement_type, reference, user_id, reason
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Removes stock from a specific location, failing if it would go negative.
#[allow(clippy::too_many_arguments)]
pub async fn remove_stock_at(
    tx: &mut Tx<'_>,
    product_id: Uuid,
    location_id: Uuid,
    qty: Decimal,
    movement_type: &str,
    reference: Option<&str>,
    user_id: Uuid,
    reason: Option<&str>,
) -> AppResult<()> {
    if qty <= Decimal::ZERO {
        return Ok(());
    }
    let available: Decimal = sqlx::query_scalar!(
        "SELECT available FROM inventory_balances WHERE product_id = $1 AND location_id = $2 FOR UPDATE",
        product_id, location_id
    )
    .fetch_optional(&mut **tx)
    .await?
    .flatten()
    .unwrap_or(Decimal::ZERO);

    if available < qty {
        return Err(AppError::BusinessLogic(format!(
            "Insufficient stock at this location. Available: {}, requested: {}",
            available.normalize(), qty.normalize()
        )));
    }

    sqlx::query!(
        "UPDATE inventory_balances SET on_hand = on_hand - $1, updated_at = NOW() WHERE product_id = $2 AND location_id = $3",
        qty, product_id, location_id
    )
    .execute(&mut **tx)
    .await?;

    sqlx::query!(
        r#"INSERT INTO stock_movements (product_id, quantity, source_location_id, movement_type, reference_document, user_id, reason)
           VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
        product_id, qty, location_id, movement_type, reference, user_id, reason
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Moves stock between two locations as a single `rack_transfer` movement.
pub async fn transfer_stock(
    tx: &mut Tx<'_>,
    product_id: Uuid,
    from: Uuid,
    to: Uuid,
    qty: Decimal,
    user_id: Uuid,
    reason: &str,
) -> AppResult<()> {
    let available: Decimal = sqlx::query_scalar!(
        "SELECT available FROM inventory_balances WHERE product_id = $1 AND location_id = $2 FOR UPDATE",
        product_id, from
    )
    .fetch_optional(&mut **tx)
    .await?
    .flatten()
    .unwrap_or(Decimal::ZERO);
    if available < qty {
        return Err(AppError::BusinessLogic(format!(
            "Insufficient stock at the source rack. Available: {}",
            available.normalize()
        )));
    }

    sqlx::query!(
        "UPDATE inventory_balances SET on_hand = on_hand - $1, updated_at = NOW() WHERE product_id = $2 AND location_id = $3",
        qty, product_id, from
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query!(
        r#"INSERT INTO inventory_balances (product_id, location_id, on_hand) VALUES ($1, $2, $3)
           ON CONFLICT (product_id, location_id) DO UPDATE
           SET on_hand = inventory_balances.on_hand + EXCLUDED.on_hand, updated_at = NOW()"#,
        product_id, to, qty
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query!(
        r#"INSERT INTO stock_movements (product_id, quantity, source_location_id, destination_location_id, movement_type, user_id, reason)
           VALUES ($1, $2, $3, $4, 'rack_transfer', $5, $6)"#,
        product_id, qty, from, to, user_id, reason
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Removes stock for a sale, drawing from the locations with the most
/// available stock first. Fails if total available stock is insufficient.
pub async fn dispatch_stock(
    tx: &mut Tx<'_>,
    product_id: Uuid,
    product_name: &str,
    qty: Decimal,
    reference: &str,
    user_id: Uuid,
) -> AppResult<()> {
    let balances = sqlx::query!(
        r#"SELECT location_id, available AS "available!"
           FROM inventory_balances
           WHERE product_id = $1 AND available > 0
           ORDER BY available DESC
           FOR UPDATE"#,
        product_id
    )
    .fetch_all(&mut **tx)
    .await?;

    let total: Decimal = balances.iter().map(|b| b.available).sum();
    if total < qty {
        return Err(AppError::BusinessLogic(format!(
            "Insufficient stock for {}: {} available, {} requested",
            product_name, total.normalize(), qty.normalize()
        )));
    }

    let mut remaining = qty;
    for b in balances {
        if remaining <= Decimal::ZERO {
            break;
        }
        let take = remaining.min(b.available);
        sqlx::query!(
            "UPDATE inventory_balances SET on_hand = on_hand - $1, updated_at = NOW() WHERE product_id = $2 AND location_id = $3",
            take, product_id, b.location_id
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query!(
            r#"INSERT INTO stock_movements (product_id, quantity, source_location_id, movement_type, reference_document, user_id)
               VALUES ($1, $2, $3, 'sales_dispatch', $4, $5)"#,
            product_id, take, b.location_id, reference, user_id
        )
        .execute(&mut **tx)
        .await?;
        remaining -= take;
    }
    Ok(())
}

/// Reverses all sales dispatches recorded against a reference (an invoice id),
/// returning the stock to the racks it was taken from.
pub async fn reverse_dispatch(tx: &mut Tx<'_>, reference: &str, user_id: Uuid, reason: &str) -> AppResult<()> {
    let moves = sqlx::query!(
        r#"SELECT product_id, quantity, source_location_id FROM stock_movements
           WHERE reference_document = $1 AND movement_type = 'sales_dispatch'"#,
        reference
    )
    .fetch_all(&mut **tx)
    .await?;

    for m in moves {
        let location = match m.source_location_id {
            Some(l) => l,
            // Older records did not store the source rack.
            None => default_location(tx).await?,
        };
        add_stock(tx, m.product_id, location, m.quantity, "customer_return", Some(reference), user_id, Some(reason)).await?;
    }
    Ok(())
}
