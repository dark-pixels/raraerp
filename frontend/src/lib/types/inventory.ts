// Product & Inventory types
export interface Category {
  id: string;
  name: string;
  slug: string;
  parent_id?: string;
  description?: string;
  created_at: string;
}

export interface Brand {
  id: string;
  name: string;
  logo_url?: string;
  created_at: string;
}

export interface Unit {
  id: string;
  name: string;
  abbreviation: string;
}

export interface Product {
  id: string;
  sku: string;
  barcode?: string;
  name: string;
  description?: string;
  category_id: string;
  category_name?: string;
  subcategory_id?: string;
  brand_id?: string;
  brand_name?: string;
  manufacturer?: string;
  image_url?: string;
  unit_id: string;
  unit_name?: string;
  purchase_unit_id?: string;
  sales_unit_id?: string;
  unit_conversion_factor: number;
  purchase_price: string;
  retail_price: string;
  wholesale_price: string;
  min_selling_price?: string;
  gst_rate: string;
  hsn_code?: string;
  reorder_threshold: number;
  reorder_quantity: number;
  primary_supplier_id?: string;
  status: 'active' | 'inactive' | 'discontinued';
  created_at: string;
  updated_at: string;
  // inventory computed
  total_stock?: number;
  available_stock?: number;
  reserved_stock?: number;
  damaged_stock?: number;
}

export interface InventoryBalance {
  id: string;
  product_id: string;
  product_name?: string;
  sku?: string;
  location_id: string;
  location_code?: string;
  warehouse_name?: string;
  rack_code?: string;
  on_hand: number;
  reserved: number;
  available: number;
  damaged: number;
  quarantined: number;
  updated_at: string;
}

export interface Warehouse {
  id: string;
  name: string;
  code: string;
  address?: string;
  is_active: boolean;
  created_at: string;
}

export interface Zone {
  id: string;
  warehouse_id: string;
  name: string;
  code: string;
}

export interface RackLocation {
  id: string;
  warehouse_id: string;
  warehouse_name?: string;
  zone_id?: string;
  zone_name?: string;
  rack_code: string;
  shelf?: string;
  bin?: string;
  qr_code?: string;
  is_active: boolean;
  created_at: string;
  product_count?: number;
}

export interface StockMovement {
  id: string;
  product_id: string;
  product_name?: string;
  sku?: string;
  quantity: number;
  source_location_id?: string;
  source_location?: string;
  destination_location_id?: string;
  destination_location?: string;
  movement_type: MovementType;
  reference_document?: string;
  user_id: string;
  user_name?: string;
  approver_id?: string;
  approver_name?: string;
  timestamp: string;
  reason?: string;
  notes?: string;
}

export type MovementType =
  | 'opening_stock'
  | 'purchase_receipt'
  | 'sales_dispatch'
  | 'rack_transfer'
  | 'warehouse_transfer'
  | 'customer_return'
  | 'supplier_return'
  | 'damaged_stock'
  | 'approved_adjustment'
  | 'stock_reconciliation';

export interface InventoryImport {
  id: string;
  filename: string;
  status: 'pending' | 'processing' | 'completed' | 'failed';
  mode: 'create' | 'update' | 'opening_stock' | 'adjustment';
  total_rows: number;
  success_count: number;
  error_count: number;
  created_by: string;
  created_by_name?: string;
  created_at: string;
  completed_at?: string;
  error_file_url?: string;
}
