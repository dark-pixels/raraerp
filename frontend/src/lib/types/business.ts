// Financial & Transaction types
export interface Supplier {
  id: string;
  business_name: string;
  contact_person?: string;
  phone: string;
  email?: string;
  address?: string;
  gstin?: string;
  gst_verified?: boolean;
  gst_status?: GstStatus;
  payment_terms_days: number;
  credit_limit: string;
  opening_balance: string;
  total_purchases: string;
  total_paid: string;
  outstanding_balance: string;
  notes?: string;
  is_active: boolean;
  created_at: string;
}

export interface Customer {
  id: string;
  name: string;
  business_name?: string;
  phone: string;
  email?: string;
  billing_address?: string;
  shipping_address?: string;
  gstin?: string;
  gst_verified?: boolean;
  gst_status?: GstStatus;
  customer_type: 'retail' | 'wholesale';
  credit_limit: string;
  credit_period_days: number;
  opening_balance: string;
  total_purchases: string;
  total_paid: string;
  outstanding_balance: string;
  notes?: string;
  is_active: boolean;
  created_at: string;
}

export interface PurchaseOrder {
  id: string;
  po_number: string;
  supplier_id: string;
  supplier_name?: string;
  supplier_gstin?: string;
  supplier_invoice_number?: string;
  purchase_date: string;
  invoice_date?: string;
  items: PurchaseOrderItem[];
  subtotal: string;
  discount_amount: string;
  taxable_value: string;
  gst_amount: string;
  freight: string;
  other_charges: string;
  grand_total: string;
  advance_paid: string;
  outstanding_amount: string;
  payment_terms_days: number;
  due_date?: string;
  status: PurchaseStatus;
  payment_status: PaymentStatus;
  notes?: string;
  created_by: string;
  created_at: string;
  updated_at: string;
}

export interface PurchaseOrderItem {
  id: string;
  product_id: string;
  product_name?: string;
  sku?: string;
  quantity: number;
  unit_id: string;
  unit_name?: string;
  unit_cost: string;
  discount_percent: string;
  taxable_value: string;
  gst_rate: string;
  gst_amount: string;
  total: string;
  received_quantity: number;
}

export type PurchaseStatus =
  | 'draft' | 'approved' | 'ordered' | 'partially_received'
  | 'received' | 'cancelled';

export type PaymentStatus =
  | 'unpaid' | 'partially_paid' | 'paid' | 'overdue';

export interface SalesInvoice {
  id: string;
  invoice_number: string;
  customer_id?: string;
  customer_name?: string;
  customer_gstin?: string;
  invoice_date: string;
  due_date?: string;
  items: SalesInvoiceItem[];
  subtotal: string;
  discount_amount: string;
  taxable_value: string;
  cgst_amount: string;
  sgst_amount: string;
  igst_amount: string;
  other_charges: string;
  grand_total: string;
  amount_paid: string;
  outstanding_amount: string;
  status: InvoiceStatus;
  payment_method?: PaymentMethod;
  notes?: string;
  created_by: string;
  created_at: string;
  updated_at: string;
}

export interface SalesInvoiceItem {
  id: string;
  product_id: string;
  product_name?: string;
  sku?: string;
  hsn_code?: string;
  quantity: number;
  unit_id: string;
  unit_name?: string;
  unit_price: string;
  discount_percent: string;
  taxable_value: string;
  gst_rate: string;
  cgst_amount: string;
  sgst_amount: string;
  igst_amount: string;
  total: string;
}

export type InvoiceStatus =
  | 'draft' | 'issued' | 'partially_paid' | 'paid'
  | 'overdue' | 'cancelled' | 'returned';

export type PaymentMethod = 'cash' | 'bank_transfer' | 'razorpay' | 'upi' | 'cheque';

export interface Payment {
  id: string;
  payment_date: string;
  amount: string;
  currency: string;
  payment_method: PaymentMethod;
  direction: 'inbound' | 'outbound';
  reference_type?: 'invoice' | 'purchase';
  reference_id?: string;
  customer_id?: string;
  supplier_id?: string;
  transaction_reference?: string;
  gateway_reference?: string;
  status: 'pending' | 'verified' | 'failed' | 'reversed';
  recorded_by: string;
  verified_by?: string;
  notes?: string;
  created_at: string;
}

export interface Expense {
  id: string;
  category: string;
  amount: string;
  date: string;
  payment_method: PaymentMethod;
  reference?: string;
  description?: string;
  attachment_url?: string;
  creator_id: string;
  approval_status: 'pending' | 'approved' | 'rejected';
  created_at: string;
}

export type GstStatus = 'active' | 'suspended' | 'cancelled' | 'pending' | 'unknown';

export interface GstVerification {
  id: string;
  gstin: string;
  status: GstStatus;
  legal_name?: string;
  trade_name?: string;
  registration_type?: string;
  state?: string;
  registration_date?: string;
  cancellation_date?: string;
  verified_at: string;
  is_cached: boolean;
  source: string;
}

export interface UpiPaymentDetails {
  upi_id: string;
  merchant_name: string;
  amount: string;
  reference: string;
  note: string;
  qr_string: string;
}

export interface RazorpayOrder {
  id: string;
  razorpay_order_id: string;
  amount: number;
  currency: string;
  status: string;
  invoice_id: string;
}

export interface DashboardSummary {
  total_products: number;
  total_stock_value: string;
  low_stock_count: number;
  out_of_stock_count: number;
  today_sales: string;
  today_purchases: string;
  monthly_sales: string;
  monthly_purchases: string;
  customer_receivables: string;
  supplier_payables: string;
  payments_due_10_days: string;
  overdue_supplier_payments: string;
  overdue_customer_invoices: string;
  today_collected: string;
  pending_adjustments: number;
  pending_verifications: number;
}

export interface Notification {
  id: string;
  type: string;
  title: string;
  body: string;
  reference_type?: string;
  reference_id?: string;
  is_read: boolean;
  created_at: string;
}

export interface AuditLog {
  id: string;
  user_id: string;
  user_name?: string;
  action: string;
  resource_type: string;
  resource_id?: string;
  changes?: Record<string, unknown>;
  ip_address?: string;
  created_at: string;
}

// Pagination
export interface PaginatedResponse<T> {
  data: T[];
  total: number;
  page: number;
  per_page: number;
  total_pages: number;
}

export interface ApiError {
  error: string;
  message: string;
  field_errors?: Record<string, string[]>;
}
