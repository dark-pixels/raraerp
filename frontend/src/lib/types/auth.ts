// Auth & User types
export type UserRole = 'super_admin' | 'admin' | 'staff';

export interface AuthUser {
  id: string;
  email: string;
  full_name: string;
  role: UserRole;
  permissions: string[];
  is_active: boolean;
  avatar_url?: string;
  last_login?: string;
}

export interface LoginRequest {
  email: string;
  password: string;
}

export interface LoginResponse {
  access_token: string;
  refresh_token: string;
  expires_in: number;
  user: AuthUser;
}

export interface RefreshResponse {
  access_token: string;
  refresh_token: string;
  expires_in: number;
}

export type Permission =
  | 'users.read' | 'users.create' | 'users.update' | 'users.deactivate'
  | 'roles.manage'
  | 'inventory.read' | 'inventory.create' | 'inventory.update'
  | 'inventory.import' | 'inventory.adjust' | 'inventory.count'
  | 'inventory.rack.assign' | 'inventory.transfer'
  | 'purchases.read' | 'purchases.create' | 'purchases.approve' | 'purchases.receive'
  | 'sales.create' | 'invoices.create' | 'invoices.cancel'
  | 'payments.record' | 'payments.approve'
  | 'finance.read' | 'reports.export'
  | 'gst.verify' | 'settings.manage';
