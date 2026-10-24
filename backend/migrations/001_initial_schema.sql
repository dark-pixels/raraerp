-- RARA Enterprise ERP - Complete Database Schema
-- Migration: 001_initial_schema.sql

-- Enable required extensions
CREATE EXTENSION IF NOT EXISTS "pgcrypto";
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- ═══════════════════════════════════════════════════
-- IDENTITY & AUTH
-- ═══════════════════════════════════════════════════

CREATE TABLE users (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email           TEXT NOT NULL UNIQUE,
    full_name       TEXT NOT NULL,
    password_hash   TEXT,
    role            TEXT NOT NULL CHECK (role IN ('super_admin', 'admin', 'staff')),
    is_active       BOOLEAN NOT NULL DEFAULT true,
    avatar_url      TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE permissions (
    id      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    key     TEXT NOT NULL UNIQUE,
    label   TEXT NOT NULL,
    description TEXT
);

CREATE TABLE role_permissions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    role            TEXT NOT NULL CHECK (role IN ('super_admin', 'admin', 'staff')),
    permission_id   UUID NOT NULL REFERENCES permissions(id) ON DELETE CASCADE,
    UNIQUE(role, permission_id)
);

CREATE TABLE user_permission_overrides (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    permission_id   UUID NOT NULL REFERENCES permissions(id) ON DELETE CASCADE,
    granted         BOOLEAN NOT NULL DEFAULT true,
    UNIQUE(user_id, permission_id)
);

CREATE TABLE refresh_tokens (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash  TEXT NOT NULL UNIQUE,
    expires_at  TIMESTAMPTZ NOT NULL,
    revoked_at  TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE login_events (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    ip_address      TEXT,
    user_agent      TEXT,
    success         BOOLEAN NOT NULL,
    failure_reason  TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ═══════════════════════════════════════════════════
-- PRODUCTS & INVENTORY
-- ═══════════════════════════════════════════════════

CREATE TABLE categories (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name        TEXT NOT NULL,
    slug        TEXT NOT NULL UNIQUE,
    parent_id   UUID REFERENCES categories(id),
    description TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE brands (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name        TEXT NOT NULL UNIQUE,
    logo_url    TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE units_of_measure (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name            TEXT NOT NULL UNIQUE,
    abbreviation    TEXT NOT NULL
);

CREATE TABLE products (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sku                     TEXT NOT NULL UNIQUE,
    barcode                 TEXT UNIQUE,
    name                    TEXT NOT NULL,
    description             TEXT,
    category_id             UUID NOT NULL REFERENCES categories(id),
    brand_id                UUID REFERENCES brands(id),
    manufacturer            TEXT,
    image_url               TEXT,
    unit_id                 UUID NOT NULL REFERENCES units_of_measure(id),
    purchase_unit_id        UUID REFERENCES units_of_measure(id),
    sales_unit_id           UUID REFERENCES units_of_measure(id),
    unit_conversion_factor  NUMERIC(10,4) NOT NULL DEFAULT 1,
    purchase_price          NUMERIC(15,2) NOT NULL DEFAULT 0,
    retail_price            NUMERIC(15,2) NOT NULL DEFAULT 0,
    wholesale_price         NUMERIC(15,2) NOT NULL DEFAULT 0,
    min_selling_price       NUMERIC(15,2),
    gst_rate                NUMERIC(5,2) NOT NULL DEFAULT 18,
    hsn_code                TEXT,
    reorder_threshold       INTEGER NOT NULL DEFAULT 10,
    reorder_quantity        INTEGER NOT NULL DEFAULT 50,
    primary_supplier_id     UUID,
    status                  TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'inactive', 'discontinued')),
    created_by              UUID REFERENCES users(id),
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE product_barcodes (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    product_id  UUID NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    barcode     TEXT NOT NULL UNIQUE,
    is_primary  BOOLEAN NOT NULL DEFAULT false
);

CREATE TABLE product_images (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    product_id  UUID REFERENCES products(id) ON DELETE CASCADE,
    public_id   TEXT NOT NULL,
    url         TEXT NOT NULL,
    is_primary  BOOLEAN NOT NULL DEFAULT true,
    uploaded_by UUID REFERENCES users(id),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE warehouses (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name        TEXT NOT NULL,
    code        TEXT NOT NULL UNIQUE,
    address     TEXT,
    is_active   BOOLEAN NOT NULL DEFAULT true,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE warehouse_zones (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    warehouse_id    UUID NOT NULL REFERENCES warehouses(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    code            TEXT NOT NULL,
    UNIQUE(warehouse_id, code)
);

CREATE TABLE rack_locations (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    warehouse_id    UUID NOT NULL REFERENCES warehouses(id) ON DELETE CASCADE,
    zone_id         UUID REFERENCES warehouse_zones(id),
    rack_code       TEXT NOT NULL,
    shelf           TEXT,
    bin             TEXT,
    qr_code         TEXT,
    is_active       BOOLEAN NOT NULL DEFAULT true,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(warehouse_id, rack_code, shelf, bin)
);

CREATE TABLE inventory_balances (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    product_id      UUID NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    location_id     UUID NOT NULL REFERENCES rack_locations(id) ON DELETE CASCADE,
    on_hand         NUMERIC(15,4) NOT NULL DEFAULT 0,
    reserved        NUMERIC(15,4) NOT NULL DEFAULT 0,
    available       NUMERIC(15,4) GENERATED ALWAYS AS (on_hand - reserved) STORED,
    damaged         NUMERIC(15,4) NOT NULL DEFAULT 0,
    quarantined     NUMERIC(15,4) NOT NULL DEFAULT 0,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(product_id, location_id),
    CONSTRAINT non_negative_stock CHECK (on_hand >= 0 AND reserved >= 0 AND damaged >= 0)
);

CREATE TABLE stock_movements (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    product_id              UUID NOT NULL REFERENCES products(id),
    quantity                NUMERIC(15,4) NOT NULL,
    source_location_id      UUID REFERENCES rack_locations(id),
    destination_location_id UUID REFERENCES rack_locations(id),
    movement_type           TEXT NOT NULL CHECK (movement_type IN (
        'opening_stock', 'purchase_receipt', 'sales_dispatch', 'rack_transfer',
        'warehouse_transfer', 'customer_return', 'supplier_return',
        'damaged_stock', 'approved_adjustment', 'stock_reconciliation'
    )),
    reference_document      TEXT,
    user_id                 UUID NOT NULL REFERENCES users(id),
    approver_id             UUID REFERENCES users(id),
    timestamp               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    reason                  TEXT,
    notes                   TEXT
);

CREATE TABLE stock_counts (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    location_id     UUID NOT NULL REFERENCES rack_locations(id),
    status          TEXT NOT NULL DEFAULT 'in_progress' CHECK (status IN ('in_progress', 'submitted', 'approved', 'rejected')),
    created_by      UUID NOT NULL REFERENCES users(id),
    approved_by     UUID REFERENCES users(id),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at    TIMESTAMPTZ
);

CREATE TABLE stock_count_items (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    count_id        UUID NOT NULL REFERENCES stock_counts(id) ON DELETE CASCADE,
    product_id      UUID NOT NULL REFERENCES products(id),
    system_quantity NUMERIC(15,4) NOT NULL,
    counted_quantity NUMERIC(15,4),
    discrepancy     NUMERIC(15,4) GENERATED ALWAYS AS (counted_quantity - system_quantity) STORED
);

CREATE TABLE inventory_imports (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    filename        TEXT NOT NULL,
    mode            TEXT NOT NULL CHECK (mode IN ('create', 'update', 'opening_stock', 'adjustment')),
    status          TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'processing', 'completed', 'failed')),
    total_rows      INTEGER NOT NULL DEFAULT 0,
    success_count   INTEGER NOT NULL DEFAULT 0,
    error_count     INTEGER NOT NULL DEFAULT 0,
    created_by      UUID NOT NULL REFERENCES users(id),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at    TIMESTAMPTZ,
    error_file_url  TEXT
);

CREATE TABLE inventory_import_errors (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    import_id       UUID NOT NULL REFERENCES inventory_imports(id) ON DELETE CASCADE,
    row_number      INTEGER NOT NULL,
    error_message   TEXT NOT NULL,
    raw_data        TEXT
);

-- ═══════════════════════════════════════════════════
-- SUPPLIERS & CUSTOMERS
-- ═══════════════════════════════════════════════════

CREATE TABLE suppliers (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    business_name       TEXT NOT NULL,
    contact_person      TEXT,
    phone               TEXT NOT NULL,
    email               TEXT,
    address             TEXT,
    gstin               TEXT,
    gst_verified        BOOLEAN DEFAULT false,
    gst_status          TEXT DEFAULT 'pending',
    payment_terms_days  INTEGER NOT NULL DEFAULT 30,
    credit_limit        NUMERIC(15,2) NOT NULL DEFAULT 0,
    opening_balance     NUMERIC(15,2) NOT NULL DEFAULT 0,
    outstanding_balance NUMERIC(15,2) NOT NULL DEFAULT 0,
    notes               TEXT,
    is_active           BOOLEAN NOT NULL DEFAULT true,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE customers (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name                TEXT NOT NULL,
    business_name       TEXT,
    phone               TEXT NOT NULL,
    email               TEXT,
    billing_address     TEXT,
    shipping_address    TEXT,
    gstin               TEXT,
    gst_verified        BOOLEAN DEFAULT false,
    gst_status          TEXT DEFAULT 'pending',
    customer_type       TEXT NOT NULL DEFAULT 'retail' CHECK (customer_type IN ('retail', 'wholesale')),
    credit_limit        NUMERIC(15,2) NOT NULL DEFAULT 0,
    credit_period_days  INTEGER NOT NULL DEFAULT 0,
    opening_balance     NUMERIC(15,2) NOT NULL DEFAULT 0,
    outstanding_balance NUMERIC(15,2) NOT NULL DEFAULT 0,
    notes               TEXT,
    is_active           BOOLEAN NOT NULL DEFAULT true,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ═══════════════════════════════════════════════════
-- PURCHASES
-- ═══════════════════════════════════════════════════

CREATE TABLE purchase_orders (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    po_number               TEXT NOT NULL UNIQUE,
    supplier_id             UUID NOT NULL REFERENCES suppliers(id),
    supplier_invoice_number TEXT,
    purchase_date           DATE NOT NULL DEFAULT CURRENT_DATE,
    invoice_date            DATE,
    subtotal                NUMERIC(15,2) NOT NULL DEFAULT 0,
    discount_amount         NUMERIC(15,2) NOT NULL DEFAULT 0,
    taxable_value           NUMERIC(15,2) NOT NULL DEFAULT 0,
    gst_amount              NUMERIC(15,2) NOT NULL DEFAULT 0,
    freight                 NUMERIC(15,2) NOT NULL DEFAULT 0,
    other_charges           NUMERIC(15,2) NOT NULL DEFAULT 0,
    grand_total             NUMERIC(15,2) NOT NULL DEFAULT 0,
    advance_paid            NUMERIC(15,2) NOT NULL DEFAULT 0,
    outstanding_amount      NUMERIC(15,2) NOT NULL DEFAULT 0,
    payment_terms_days      INTEGER NOT NULL DEFAULT 30,
    due_date                DATE,
    status                  TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'approved', 'ordered', 'partially_received', 'received', 'cancelled')),
    payment_status          TEXT NOT NULL DEFAULT 'unpaid' CHECK (payment_status IN ('unpaid', 'partially_paid', 'paid', 'overdue')),
    notes                   TEXT,
    created_by              UUID NOT NULL REFERENCES users(id),
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE purchase_order_items (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    purchase_order_id   UUID NOT NULL REFERENCES purchase_orders(id) ON DELETE CASCADE,
    product_id          UUID NOT NULL REFERENCES products(id),
    quantity            NUMERIC(15,4) NOT NULL,
    unit_id             UUID REFERENCES units_of_measure(id),
    unit_cost           NUMERIC(15,2) NOT NULL,
    discount_percent    NUMERIC(5,2) NOT NULL DEFAULT 0,
    taxable_value       NUMERIC(15,2) NOT NULL,
    gst_rate            NUMERIC(5,2) NOT NULL DEFAULT 18,
    gst_amount          NUMERIC(15,2) NOT NULL,
    total               NUMERIC(15,2) NOT NULL,
    received_quantity   NUMERIC(15,4) NOT NULL DEFAULT 0
);

-- ═══════════════════════════════════════════════════
-- SALES & INVOICES
-- ═══════════════════════════════════════════════════

CREATE TABLE sales_invoices (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    invoice_number      TEXT NOT NULL UNIQUE,
    customer_id         UUID REFERENCES customers(id),
    invoice_date        DATE NOT NULL DEFAULT CURRENT_DATE,
    due_date            DATE,
    subtotal            NUMERIC(15,2) NOT NULL DEFAULT 0,
    discount_amount     NUMERIC(15,2) NOT NULL DEFAULT 0,
    taxable_value       NUMERIC(15,2) NOT NULL DEFAULT 0,
    cgst_amount         NUMERIC(15,2) NOT NULL DEFAULT 0,
    sgst_amount         NUMERIC(15,2) NOT NULL DEFAULT 0,
    igst_amount         NUMERIC(15,2) NOT NULL DEFAULT 0,
    other_charges       NUMERIC(15,2) NOT NULL DEFAULT 0,
    grand_total         NUMERIC(15,2) NOT NULL DEFAULT 0,
    amount_paid         NUMERIC(15,2) NOT NULL DEFAULT 0,
    outstanding_amount  NUMERIC(15,2) NOT NULL DEFAULT 0,
    status              TEXT NOT NULL DEFAULT 'issued' CHECK (status IN ('draft', 'issued', 'partially_paid', 'paid', 'overdue', 'cancelled', 'returned')),
    payment_method      TEXT,
    notes               TEXT,
    created_by          UUID NOT NULL REFERENCES users(id),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE sales_invoice_items (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    invoice_id      UUID NOT NULL REFERENCES sales_invoices(id) ON DELETE CASCADE,
    product_id      UUID NOT NULL REFERENCES products(id),
    quantity        NUMERIC(15,4) NOT NULL,
    unit_id         UUID REFERENCES units_of_measure(id),
    unit_price      NUMERIC(15,2) NOT NULL,
    discount_percent NUMERIC(5,2) NOT NULL DEFAULT 0,
    taxable_value   NUMERIC(15,2) NOT NULL,
    gst_rate        NUMERIC(5,2) NOT NULL DEFAULT 18,
    cgst_amount     NUMERIC(15,2) NOT NULL DEFAULT 0,
    sgst_amount     NUMERIC(15,2) NOT NULL DEFAULT 0,
    igst_amount     NUMERIC(15,2) NOT NULL DEFAULT 0,
    total           NUMERIC(15,2) NOT NULL
);

CREATE TABLE credit_notes (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    original_invoice_id UUID REFERENCES sales_invoices(id),
    total               NUMERIC(15,2) NOT NULL,
    reason              TEXT,
    status              TEXT NOT NULL DEFAULT 'draft',
    created_by          UUID REFERENCES users(id),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ═══════════════════════════════════════════════════
-- PAYMENTS
-- ═══════════════════════════════════════════════════

CREATE TABLE payments (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    payment_date            DATE NOT NULL DEFAULT CURRENT_DATE,
    amount                  NUMERIC(15,2) NOT NULL,
    currency                TEXT NOT NULL DEFAULT 'INR',
    payment_method          TEXT NOT NULL CHECK (payment_method IN ('cash', 'bank_transfer', 'razorpay', 'upi', 'cheque')),
    direction               TEXT NOT NULL CHECK (direction IN ('inbound', 'outbound')),
    reference_type          TEXT CHECK (reference_type IN ('invoice', 'purchase')),
    reference_id            UUID,
    customer_id             UUID REFERENCES customers(id),
    supplier_id             UUID REFERENCES suppliers(id),
    transaction_reference   TEXT,
    gateway_reference       TEXT UNIQUE,
    status                  TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'verified', 'failed', 'reversed')),
    recorded_by             UUID REFERENCES users(id),
    verified_by             UUID REFERENCES users(id),
    notes                   TEXT,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE payment_gateway_orders (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    invoice_id          UUID REFERENCES sales_invoices(id),
    gateway             TEXT NOT NULL,
    gateway_order_id    TEXT NOT NULL UNIQUE,
    gateway_payment_id  TEXT,
    amount              NUMERIC(15,2) NOT NULL,
    currency            TEXT NOT NULL DEFAULT 'INR',
    status              TEXT NOT NULL DEFAULT 'created',
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE payment_gateway_events (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    gateway     TEXT NOT NULL,
    event_type  TEXT NOT NULL,
    event_id    TEXT UNIQUE,
    payload     JSONB,
    processed   BOOLEAN NOT NULL DEFAULT false,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ═══════════════════════════════════════════════════
-- FINANCE
-- ═══════════════════════════════════════════════════

CREATE TABLE expenses (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    category        TEXT NOT NULL,
    amount          NUMERIC(15,2) NOT NULL,
    date            DATE NOT NULL DEFAULT CURRENT_DATE,
    payment_method  TEXT NOT NULL DEFAULT 'cash',
    reference       TEXT,
    description     TEXT,
    attachment_url  TEXT,
    created_by      UUID REFERENCES users(id),
    approved_by     UUID REFERENCES users(id),
    approval_status TEXT NOT NULL DEFAULT 'pending' CHECK (approval_status IN ('pending', 'approved', 'rejected')),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ═══════════════════════════════════════════════════
-- GST & INTEGRATIONS
-- ═══════════════════════════════════════════════════

CREATE TABLE gst_verification_records (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    gstin               TEXT NOT NULL,
    status              TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('active', 'suspended', 'cancelled', 'pending', 'unknown')),
    legal_name          TEXT,
    trade_name          TEXT,
    registration_type   TEXT,
    state_jurisdiction  TEXT,
    registration_date   DATE,
    cancellation_date   DATE,
    verified_by         UUID REFERENCES users(id),
    verified_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    is_cached           BOOLEAN NOT NULL DEFAULT false,
    source              TEXT NOT NULL DEFAULT 'api'
);

CREATE TABLE integration_settings (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    key         TEXT NOT NULL UNIQUE,
    value       TEXT,
    is_encrypted BOOLEAN NOT NULL DEFAULT false,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ═══════════════════════════════════════════════════
-- NOTIFICATIONS & JOBS
-- ═══════════════════════════════════════════════════

CREATE TABLE notifications (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    type            TEXT NOT NULL,
    title           TEXT NOT NULL,
    body            TEXT NOT NULL,
    reference_type  TEXT,
    reference_id    TEXT,
    is_read         BOOLEAN NOT NULL DEFAULT false,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE payment_reminders (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    reference_type          TEXT NOT NULL CHECK (reference_type IN ('invoice', 'purchase')),
    reference_id            UUID NOT NULL,
    due_date                DATE,
    outstanding_amount      NUMERIC(15,2) NOT NULL,
    status                  TEXT NOT NULL DEFAULT 'scheduled' CHECK (status IN ('scheduled', 'notified', 'overdue', 'settled', 'cancelled')),
    reminder_days_before    INTEGER NOT NULL DEFAULT 2,
    last_notified_at        TIMESTAMPTZ,
    next_reminder_at        TIMESTAMPTZ,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ═══════════════════════════════════════════════════
-- AUDIT LOGS
-- ═══════════════════════════════════════════════════

CREATE TABLE audit_logs (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id),
    action          TEXT NOT NULL,
    resource_type   TEXT NOT NULL,
    resource_id     TEXT,
    changes         JSONB,
    ip_address      TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ═══════════════════════════════════════════════════
-- COMPANY SETTINGS
-- ═══════════════════════════════════════════════════

CREATE TABLE company_settings (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name            TEXT NOT NULL DEFAULT 'RARA Enterprise',
    gstin           TEXT,
    address         TEXT,
    phone           TEXT,
    email           TEXT,
    logo_url        TEXT,
    invoice_prefix  TEXT NOT NULL DEFAULT 'INV',
    upi_id          TEXT,
    upi_name        TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ═══════════════════════════════════════════════════
-- INDEXES
-- ═══════════════════════════════════════════════════

CREATE INDEX idx_products_sku ON products(sku);
CREATE INDEX idx_products_barcode ON products(barcode);
CREATE INDEX idx_products_category ON products(category_id);
CREATE INDEX idx_products_status ON products(status);
CREATE INDEX idx_inventory_balances_product ON inventory_balances(product_id);
CREATE INDEX idx_inventory_balances_location ON inventory_balances(location_id);
CREATE INDEX idx_stock_movements_product ON stock_movements(product_id);
CREATE INDEX idx_stock_movements_timestamp ON stock_movements(timestamp DESC);
CREATE INDEX idx_sales_invoices_customer ON sales_invoices(customer_id);
CREATE INDEX idx_sales_invoices_date ON sales_invoices(invoice_date DESC);
CREATE INDEX idx_sales_invoices_status ON sales_invoices(status);
CREATE INDEX idx_purchase_orders_supplier ON purchase_orders(supplier_id);
CREATE INDEX idx_purchase_orders_status ON purchase_orders(status);
CREATE INDEX idx_payments_customer ON payments(customer_id);
CREATE INDEX idx_payments_supplier ON payments(supplier_id);
CREATE INDEX idx_payments_date ON payments(payment_date DESC);
CREATE INDEX idx_notifications_user ON notifications(user_id);
CREATE INDEX idx_notifications_read ON notifications(user_id, is_read);
CREATE INDEX idx_audit_logs_user ON audit_logs(user_id);
CREATE INDEX idx_audit_logs_created ON audit_logs(created_at DESC);
CREATE INDEX idx_refresh_tokens_hash ON refresh_tokens(token_hash);
CREATE INDEX idx_refresh_tokens_user ON refresh_tokens(user_id);
CREATE INDEX idx_gst_verifications_gstin ON gst_verification_records(gstin);
CREATE INDEX idx_payment_reminders_status ON payment_reminders(status);

-- ═══════════════════════════════════════════════════
-- SEED DATA
-- ═══════════════════════════════════════════════════

-- Default permissions
INSERT INTO permissions (key, label, description) VALUES
    ('users.read', 'View Users', 'Can view user list and details'),
    ('users.create', 'Create Users', 'Can create new user accounts'),
    ('users.update', 'Update Users', 'Can update user details'),
    ('users.deactivate', 'Deactivate Users', 'Can deactivate user accounts'),
    ('roles.manage', 'Manage Roles', 'Can assign roles and permissions'),
    ('inventory.read', 'View Inventory', 'Can view product inventory'),
    ('inventory.create', 'Create Products', 'Can create new products'),
    ('inventory.update', 'Update Products', 'Can update product details'),
    ('inventory.import', 'Import Inventory', 'Can bulk import inventory'),
    ('inventory.adjust', 'Adjust Stock', 'Can approve stock adjustments'),
    ('inventory.count', 'Stock Count', 'Can perform physical stock counts'),
    ('inventory.rack.assign', 'Rack Assignment', 'Can assign products to rack locations'),
    ('inventory.transfer', 'Stock Transfer', 'Can transfer stock between locations'),
    ('purchases.read', 'View Purchases', 'Can view purchase orders'),
    ('purchases.create', 'Create Purchases', 'Can create purchase orders'),
    ('purchases.approve', 'Approve Purchases', 'Can approve purchase orders'),
    ('purchases.receive', 'Receive Goods', 'Can record goods received'),
    ('sales.create', 'Create Sales', 'Can create sales invoices'),
    ('invoices.create', 'Manage Invoices', 'Can create and manage invoices'),
    ('invoices.cancel', 'Cancel Invoices', 'Can cancel issued invoices'),
    ('payments.record', 'Record Payments', 'Can record payments'),
    ('payments.approve', 'Approve Payments', 'Can approve payment verification'),
    ('finance.read', 'View Finance', 'Can view financial reports and data'),
    ('reports.export', 'Export Reports', 'Can export reports'),
    ('gst.verify', 'GST Verification', 'Can verify GSTINs'),
    ('settings.manage', 'Manage Settings', 'Can manage system settings')
ON CONFLICT (key) DO NOTHING;

-- Default admin permissions
INSERT INTO role_permissions (role, permission_id)
SELECT 'admin', id FROM permissions
WHERE key IN (
    'inventory.read', 'inventory.create', 'inventory.update', 'inventory.import',
    'inventory.adjust', 'inventory.count', 'inventory.rack.assign', 'inventory.transfer',
    'purchases.read', 'purchases.create', 'purchases.approve', 'purchases.receive',
    'sales.create', 'invoices.create', 'invoices.cancel',
    'payments.record', 'payments.approve',
    'finance.read', 'reports.export', 'gst.verify',
    'users.read', 'users.create', 'users.update'
)
ON CONFLICT DO NOTHING;

-- Default staff permissions
INSERT INTO role_permissions (role, permission_id)
SELECT 'staff', id FROM permissions
WHERE key IN (
    'inventory.read', 'inventory.count', 'inventory.rack.assign',
    'gst.verify', 'sales.create', 'invoices.create', 'payments.record'
)
ON CONFLICT DO NOTHING;

-- Default units
INSERT INTO units_of_measure (name, abbreviation) VALUES
    ('Piece', 'Pcs'), ('Box', 'Box'), ('Pack', 'Pck'),
    ('Set', 'Set'), ('Pair', 'Pr'), ('Dozen', 'Doz'),
    ('Kilogram', 'Kg'), ('Gram', 'g'), ('Litre', 'L'), ('Metre', 'm')
ON CONFLICT (name) DO NOTHING;

-- Default categories for furniture hardware
INSERT INTO categories (name, slug) VALUES
    ('Hinges', 'hinges'),
    ('Drawer Channels', 'drawer-channels'),
    ('Cabinet Handles', 'cabinet-handles'),
    ('Furniture Handles', 'furniture-handles'),
    ('Locks', 'locks'),
    ('Screws & Fasteners', 'screws-fasteners'),
    ('Brackets & Connectors', 'brackets-connectors'),
    ('Adhesives', 'adhesives'),
    ('Furniture Legs', 'furniture-legs'),
    ('Sliding Door Fittings', 'sliding-door-fittings'),
    ('Cabinet Fittings', 'cabinet-fittings'),
    ('Other Hardware', 'other-hardware')
ON CONFLICT (slug) DO NOTHING;

-- Default warehouse
INSERT INTO warehouses (name, code, address) VALUES
    ('Main Shop', 'MAIN', 'RARA Enterprise Main Location')
ON CONFLICT (code) DO NOTHING;

-- Company settings
INSERT INTO company_settings (name) VALUES ('RARA Enterprise')
ON CONFLICT DO NOTHING;

-- Super Admin account (password: Admin@1234 - CHANGE IN PRODUCTION)
-- Password hash for 'Admin@1234' using Argon2id
INSERT INTO users (email, full_name, password_hash, role)
VALUES (
    'admin@raraenterprise.com',
    'Super Administrator',
    '$argon2id$v=19$m=65536,t=3,p=4$placeholder_change_this',
    'super_admin'
)
ON CONFLICT (email) DO NOTHING;
