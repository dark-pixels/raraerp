-- Indexes supporting document lookups, ledgers and reminders.
-- Additive only: safe to apply to an existing database.

CREATE INDEX IF NOT EXISTS idx_stock_movements_reference ON stock_movements(reference_document);
CREATE INDEX IF NOT EXISTS idx_payments_reference ON payments(reference_type, reference_id);
CREATE INDEX IF NOT EXISTS idx_payment_reminders_reference ON payment_reminders(reference_type, reference_id);
CREATE INDEX IF NOT EXISTS idx_login_events_user_time ON login_events(user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_credit_notes_invoice ON credit_notes(original_invoice_id);
CREATE INDEX IF NOT EXISTS idx_sales_invoice_items_invoice ON sales_invoice_items(invoice_id);
CREATE INDEX IF NOT EXISTS idx_purchase_order_items_po ON purchase_order_items(purchase_order_id);
CREATE INDEX IF NOT EXISTS idx_expenses_date ON expenses(date DESC);
CREATE INDEX IF NOT EXISTS idx_customers_phone ON customers(phone);
CREATE INDEX IF NOT EXISTS idx_sales_invoices_due ON sales_invoices(due_date) WHERE outstanding_amount > 0;
CREATE INDEX IF NOT EXISTS idx_purchase_orders_due ON purchase_orders(due_date) WHERE outstanding_amount > 0;
