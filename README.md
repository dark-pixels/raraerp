# RARA Enterprise ERP

Inventory, billing and accounting for a furniture-hardware business: POS billing with GST
invoices, purchase orders and goods receipt, rack-level stock, customer credit, supplier
payables, expenses, reports and role-based staff access.

| Part | Stack | Folder |
| --- | --- | --- |
| API | Rust · Axum 0.7 · SQLx 0.8 · PostgreSQL | `backend/` |
| Web app | SvelteKit 2 · Svelte 5 · Tailwind | `frontend/` |

## Running locally

**Backend** (needs PostgreSQL; migrations run automatically on start):

```bash
cd backend
cp .env.example .env        # then fill in DATABASE_URL and JWT_SECRET
cargo run --bin server      # http://localhost:8080
```

On Windows this project builds with the GNU toolchain and the bundled `w64devkit`
(see `start.ps1`).

SQL queries are checked at compile time against the database in `DATABASE_URL`, so a
reachable database with the current schema is required to build.

Set the first admin's password:

```bash
cargo run --bin set_password -- 'YourStrongPass1' admin@raraenterprise.com
```

**Frontend:**

```bash
cd frontend
npm install
npm run dev                 # http://localhost:5173
```

## Checks

```bash
cd backend && cargo test --bin server && cargo clippy --all-targets
cd frontend && npm run check && npm run build
```

## Configuration

All backend settings are environment variables; `backend/.env.example` documents each one.
Required in production:

- `DATABASE_URL`
- `JWT_SECRET`: at least 32 random characters. Without it, sessions do not survive restarts.
- `FRONTEND_ORIGIN`: comma-separated allowed origins.

Optional integrations: Cloudinary (product images), Razorpay (online payments and webhook),
a GST verification API, and `CRON_SECRET` for scheduled jobs.

Company name, GSTIN, address, invoice prefix and UPI ID are set in the app under
**Settings → Company profile** and are printed on invoices. The company GSTIN decides
CGST+SGST (same state) vs IGST (inter-state) on each sale.

## Scheduled jobs

Overdue marking, payment-due reminders and low-stock alerts run either:

- in the long-running worker: `cargo run --bin worker`, or
- on serverless hosts, by calling `GET /api/v1/cron/run` with
  `Authorization: Bearer $CRON_SECRET` (for example from Vercel Cron, once or twice a day).

## Business rules worth knowing

- Stock never goes negative. A sale is refused if total available stock is insufficient,
  and it draws from the racks holding the most stock first.
- Walk-in sales must be paid in full; select a customer to sell on credit. A customer's
  credit limit (when set above 0) is enforced.
- Invoices with payments cannot be cancelled. Issue a credit note instead. Cancelling an
  unpaid invoice returns its stock to the racks it came from.
- Purchase orders must be approved before goods are received, and over-receipt is refused.
  Receiving goods updates each product's cost price.
- Five failed sign-ins lock an account for 15 minutes.
