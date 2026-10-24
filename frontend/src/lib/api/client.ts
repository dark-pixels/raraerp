import type {
	ApiError,
	LoginRequest,
	LoginResponse,
	RefreshResponse,
	AuthUser
} from '$types';

const API_BASE =
	typeof window === 'undefined'
		? (process.env.BACKEND_URL ?? process.env.PUBLIC_API_BASE_URL ?? 'http://localhost:8080')
		: (import.meta.env.PUBLIC_API_BASE_URL ?? '');

class ApiClient {
	private accessToken: string | null = null;
	private refreshToken: string | null = null;
	private refreshPromise: Promise<void> | null = null;

	constructor() {
		if (typeof window !== 'undefined') {
			this.accessToken = sessionStorage.getItem('access_token');
			this.refreshToken = localStorage.getItem('refresh_token');
		}
	}

	setTokens(access: string, refresh: string) {
		this.accessToken = access;
		this.refreshToken = refresh;
		if (typeof window !== 'undefined') {
			sessionStorage.setItem('access_token', access);
			localStorage.setItem('refresh_token', refresh);
		}
	}

	clearTokens() {
		this.accessToken = null;
		this.refreshToken = null;
		if (typeof window !== 'undefined') {
			sessionStorage.removeItem('access_token');
			localStorage.removeItem('refresh_token');
		}
	}

	private async tryRefresh(): Promise<void> {
		if (!this.refreshToken) throw new Error('No refresh token');
		const res = await fetch(`${API_BASE}/api/v1/auth/refresh`, {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({ refresh_token: this.refreshToken })
		});
		if (!res.ok) {
			this.clearTokens();
			throw new Error('Session expired');
		}
		const data: RefreshResponse = await res.json();
		this.accessToken = data.access_token;
		if (typeof window !== 'undefined') {
			sessionStorage.setItem('access_token', data.access_token);
		}
	}

	async request<T>(
		path: string,
		options: RequestInit = {},
		retry = true
	): Promise<T> {
		const headers: Record<string, string> = {
			'Content-Type': 'application/json',
			...(options.headers as Record<string, string>)
		};

		if (this.accessToken) {
			headers['Authorization'] = `Bearer ${this.accessToken}`;
		}

		const res = await fetch(`${API_BASE}${path}`, { ...options, headers });

		if (res.status === 401 && retry && this.refreshToken) {
			if (!this.refreshPromise) {
				this.refreshPromise = this.tryRefresh().finally(() => {
					this.refreshPromise = null;
				});
			}
			try {
				await this.refreshPromise;
				return this.request<T>(path, options, false);
			} catch {
				this.clearTokens();
				if (typeof window !== 'undefined') {
					window.location.href = '/login';
				}
				throw new Error('Unauthorized');
			}
		}

		if (!res.ok) {
			let err: ApiError;
			try {
				err = await res.json();
			} catch {
				err = { error: 'request_failed', message: `HTTP ${res.status}` };
			}
			throw err;
		}

		if (res.status === 204) return undefined as T;
		return res.json();
	}

	get<T>(path: string, params?: Record<string, string | number | boolean | undefined>) {
		let url = path;
		if (params) {
			const q = new URLSearchParams();
			for (const [k, v] of Object.entries(params)) {
				if (v !== undefined) q.set(k, String(v));
			}
			const qs = q.toString();
			if (qs) url += `?${qs}`;
		}
		return this.request<T>(url, { method: 'GET' });
	}

	post<T>(path: string, body?: unknown) {
		return this.request<T>(path, {
			method: 'POST',
			body: body ? JSON.stringify(body) : undefined
		});
	}

	patch<T>(path: string, body?: unknown) {
		return this.request<T>(path, {
			method: 'PATCH',
			body: body ? JSON.stringify(body) : undefined
		});
	}

	put<T>(path: string, body?: unknown) {
		return this.request<T>(path, {
			method: 'PUT',
			body: body ? JSON.stringify(body) : undefined
		});
	}

	delete<T>(path: string) {
		return this.request<T>(path, { method: 'DELETE' });
	}

	async uploadFile<T>(path: string, file: File, extraFields?: Record<string, string>) {
		const form = new FormData();
		form.append('file', file);
		if (extraFields) {
			for (const [k, v] of Object.entries(extraFields)) form.append(k, v);
		}
		const headers: Record<string, string> = {};
		if (this.accessToken) headers['Authorization'] = `Bearer ${this.accessToken}`;
		const res = await fetch(`${API_BASE}${path}`, {
			method: 'POST',
			headers,
			body: form
		});
		if (!res.ok) throw await res.json().catch(() => ({ error: 'upload_failed' }));
		return res.json() as Promise<T>;
	}
}

export const api = new ApiClient();

// ── Auth API ──────────────────────────────────────────────────
export const authApi = {
	login: (data: LoginRequest) =>
		api.post<LoginResponse>('/api/v1/auth/login', data),
	logout: () => api.post<void>('/api/v1/auth/logout'),
	me: () => api.get<AuthUser>('/api/v1/auth/me'),
	refresh: (refresh_token: string) =>
		api.post<RefreshResponse>('/api/v1/auth/refresh', { refresh_token })
};

// ── Dashboard API ─────────────────────────────────────────────
export const dashboardApi = {
	summary: (params?: { from?: string; to?: string }) =>
		api.get('/api/v1/dashboard/summary', params),
	salesTrend: (params?: { period?: string }) =>
		api.get('/api/v1/dashboard/sales', params),
	activity: () => api.get('/api/v1/dashboard/activity')
};

// ── Inventory API ─────────────────────────────────────────────
export const inventoryApi = {
	// Nested namespaces for type-safe usage
	products: {
		list: (p?: Record<string, unknown>) => api.get('/api/v1/inventory/products', p as never),
		get: (id: string) => api.get(`/api/v1/inventory/products/${id}`),
		create: (data: unknown) => api.post('/api/v1/inventory/products', data),
		update: (id: string, data: unknown) => api.patch(`/api/v1/inventory/products/${id}`, data)
	},
	categories: {
		list: () => api.get('/api/v1/inventory/categories'),
		create: (data: unknown) => api.post('/api/v1/inventory/categories', data)
	},
	brands: {
		list: () => api.get('/api/v1/inventory/brands'),
		create: (data: unknown) => api.post('/api/v1/inventory/brands', data)
	},
	units: {
		list: () => api.get('/api/v1/inventory/units'),
		create: (data: unknown) => api.post('/api/v1/inventory/units', data)
	},
	imports: {
		list: (p?: Record<string, unknown>) => api.get('/api/v1/inventory/imports', p as never),
		get: (id: string) => api.get(`/api/v1/inventory/imports/${id}`),
		upload: (file: File, mode: string) => api.uploadFile('/api/v1/inventory/imports', file, { mode })
	},
	movements: (p?: Record<string, unknown>) => api.get('/api/v1/inventory/movements', p as never),
	uploadImage: (file: File, productId?: string) =>
		api.uploadFile('/api/v1/uploads/images', file, productId ? { product_id: productId } : {})
};

// ── Rack API ──────────────────────────────────────────────────
export const rackApi = {
	warehouses: () => api.get('/api/v1/warehouses'),
	createWarehouse: (data: unknown) => api.post('/api/v1/warehouses', data),
	racks: (p?: Record<string, unknown>) => api.get('/api/v1/racks', p as never),
	createRack: (data: unknown) => api.post('/api/v1/racks', data),
	rackInventory: (rackId: string) => api.get(`/api/v1/racks/${rackId}/inventory`),
	transfer: (data: unknown) => api.post('/api/v1/racks/transfers', data),
	stockCount: (data: unknown) => api.post('/api/v1/inventory/stock-counts', data)
};

// ── Purchase API ──────────────────────────────────────────────
export const purchaseApi = {
	list: (p?: Record<string, unknown>) => api.get('/api/v1/purchases', p as never),
	get: (id: string) => api.get(`/api/v1/purchases/${id}`),
	create: (data: unknown) => api.post('/api/v1/purchases', data),
	update: (id: string, data: unknown) => api.patch(`/api/v1/purchases/${id}`, data),
	receive: (id: string, data: unknown) => api.post(`/api/v1/purchases/${id}/receipts`, data),
	approve: (id: string) => api.post(`/api/v1/purchases/${id}/approve`, {})
};

// ── Supplier API ──────────────────────────────────────────────
export const supplierApi = {
	list: (p?: Record<string, unknown>) => api.get('/api/v1/suppliers', p as never),
	get: (id: string) => api.get(`/api/v1/suppliers/${id}`),
	create: (data: unknown) => api.post('/api/v1/suppliers', data),
	update: (id: string, data: unknown) => api.patch(`/api/v1/suppliers/${id}`, data),
	ledger: (id: string, p?: Record<string, unknown>) =>
		api.get(`/api/v1/suppliers/${id}/ledger`, p as never)
};

// ── Customer API ──────────────────────────────────────────────
export const customerApi = {
	list: (p?: Record<string, unknown>) => api.get('/api/v1/customers', p as never),
	get: (id: string) => api.get(`/api/v1/customers/${id}`),
	create: (data: unknown) => api.post('/api/v1/customers', data),
	update: (id: string, data: unknown) => api.patch(`/api/v1/customers/${id}`, data),
	ledger: (id: string, p?: Record<string, unknown>) =>
		api.get(`/api/v1/customers/${id}/ledger`, p as never)
};

// ── Invoice API ───────────────────────────────────────────────
export const invoiceApi = {
	list: (p?: Record<string, unknown>) => api.get('/api/v1/invoices', p as never),
	get: (id: string) => api.get(`/api/v1/invoices/${id}`),
	create: (data: unknown) => api.post('/api/v1/sales/invoices', data),
	cancel: (id: string, reason: string) =>
		api.post(`/api/v1/invoices/${id}/cancel`, { reason }),
	recordPayment: (id: string, data: unknown) =>
		api.post(`/api/v1/invoices/${id}/payments`, data),
	upiDetails: (id: string) => api.get(`/api/v1/invoices/${id}/upi-details`),
	pdf: (id: string) => `${API_BASE}/api/v1/invoices/${id}/pdf`
};

// ── Payment API ───────────────────────────────────────────────
export const paymentApi = {
	list: (p?: Record<string, unknown>) => api.get('/api/v1/payments', p as never),
	get: (id: string) => api.get(`/api/v1/payments/${id}`),
	createRazorpayOrder: (data: unknown) =>
		api.post('/api/v1/payments/razorpay/orders', data)
};

// ── Finance API ───────────────────────────────────────────────
export const financeApi = {
	payables: (p?: Record<string, unknown>) => api.get('/api/v1/finance/payables', p as never),
	receivables: (p?: Record<string, unknown>) => api.get('/api/v1/finance/receivables', p as never),
	expenses: (p?: Record<string, unknown>) => api.get('/api/v1/finance/expenses', p as never),
	createExpense: (data: unknown) => api.post('/api/v1/finance/expenses', data)
};

// ── GST API ───────────────────────────────────────────────────
export const gstApi = {
	verify: (gstin: string) => api.post('/api/v1/gst/verify', { gstin }),
	history: (p?: Record<string, unknown>) => api.get('/api/v1/gst/verifications', p as never)
};

// ── Reports API ───────────────────────────────────────────────
export const reportsApi = {
	inventory: (p?: Record<string, unknown>) => api.get('/api/v1/reports/inventory', p as never),
	sales: (p?: Record<string, unknown>) => api.get('/api/v1/reports/sales', p as never),
	purchases: (p?: Record<string, unknown>) => api.get('/api/v1/reports/purchases', p as never),
	payables: (p?: Record<string, unknown>) => api.get('/api/v1/reports/payables', p as never),
	receivables: (p?: Record<string, unknown>) =>
		api.get('/api/v1/reports/receivables', p as never)
};

// ── Notifications API ─────────────────────────────────────────
export const notificationApi = {
	list: (p?: Record<string, unknown>) => api.get('/api/v1/notifications', p as never),
	markRead: (id: string) => api.patch(`/api/v1/notifications/${id}/read`),
	markAllRead: () => api.post('/api/v1/notifications/read-all')
};

// ── Users API ─────────────────────────────────────────────────
export const usersApi = {
	list: (p?: Record<string, unknown>) => api.get('/api/v1/users', p as never),
	get: (id: string) => api.get(`/api/v1/users/${id}`),
	create: (data: unknown) => api.post('/api/v1/users', data),
	update: (id: string, data: unknown) => api.patch(`/api/v1/users/${id}`, data),
	deactivate: (id: string) => api.post(`/api/v1/users/${id}/deactivate`),
	permissions: (id: string) => api.get(`/api/v1/users/${id}/permissions`),
	updatePermissions: (id: string, permissions: string[]) =>
		api.put(`/api/v1/users/${id}/permissions`, { permissions })
};

// ── Audit API ─────────────────────────────────────────────────
export const auditApi = {
	list: (p?: Record<string, unknown>) => api.get('/api/v1/audit-logs', p as never)
};

