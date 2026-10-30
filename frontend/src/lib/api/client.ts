import type { ApiError, LoginRequest, LoginResponse, RefreshResponse, AuthUser } from '$types';

const API_BASE =
	typeof window === 'undefined'
		? (process.env.BACKEND_URL ?? process.env.PUBLIC_API_BASE_URL ?? 'http://localhost:8080')
		: (import.meta.env.PUBLIC_API_BASE_URL ?? '');

type Params = Record<string, string | number | boolean | null | undefined>;

/** Normalises any thrown value into an ApiError with a readable message. */
export function errorMessage(e: unknown, fallback = 'Something went wrong'): string {
	if (e && typeof e === 'object' && 'message' in e && typeof (e as ApiError).message === 'string') {
		return (e as ApiError).message || fallback;
	}
	return fallback;
}

function storage(kind: 'session' | 'local'): Storage | null {
	if (typeof window === 'undefined') return null;
	try {
		return kind === 'session' ? window.sessionStorage : window.localStorage;
	} catch {
		return null;
	}
}

class ApiClient {
	private accessToken: string | null = null;
	private refreshToken: string | null = null;
	private refreshPromise: Promise<void> | null = null;

	constructor() {
		this.accessToken = storage('session')?.getItem('access_token') ?? null;
		this.refreshToken = storage('local')?.getItem('refresh_token') ?? null;
	}

	setTokens(access: string, refresh?: string) {
		this.accessToken = access;
		storage('session')?.setItem('access_token', access);
		if (refresh) {
			this.refreshToken = refresh;
			storage('local')?.setItem('refresh_token', refresh);
		}
	}

	setRefreshToken(refresh: string) {
		this.refreshToken = refresh;
		storage('local')?.setItem('refresh_token', refresh);
	}

	clearTokens() {
		this.accessToken = null;
		this.refreshToken = null;
		storage('session')?.removeItem('access_token');
		storage('local')?.removeItem('refresh_token');
	}

	hasSession() {
		return !!(this.accessToken || this.refreshToken);
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
		// The server rotates refresh tokens: the old one is now revoked.
		const data: RefreshResponse = await res.json();
		this.setTokens(data.access_token, data.refresh_token);
	}

	private async send(path: string, options: RequestInit, retry: boolean): Promise<Response> {
		const isForm = typeof FormData !== 'undefined' && options.body instanceof FormData;
		const headers: Record<string, string> = {
			...(isForm || !options.body ? {} : { 'Content-Type': 'application/json' }),
			...(options.headers as Record<string, string>)
		};
		if (this.accessToken) headers['Authorization'] = `Bearer ${this.accessToken}`;

		let res: Response;
		try {
			res = await fetch(`${API_BASE}${path}`, { ...options, headers });
		} catch {
			throw { error: 'network_error', message: 'Cannot reach the server. Check your connection.' } satisfies ApiError;
		}

		if (res.status === 401 && retry && this.refreshToken) {
			if (!this.refreshPromise) {
				this.refreshPromise = this.tryRefresh().finally(() => {
					this.refreshPromise = null;
				});
			}
			try {
				await this.refreshPromise;
			} catch {
				this.clearTokens();
				if (typeof window !== 'undefined' && !window.location.pathname.startsWith('/login')) {
					window.location.href = '/login';
				}
				throw { error: 'unauthorized', message: 'Your session has expired. Please sign in again.' } satisfies ApiError;
			}
			return this.send(path, options, false);
		}

		if (!res.ok) {
			let err: ApiError;
			try {
				err = await res.json();
			} catch {
				err = { error: 'request_failed', message: `Request failed (HTTP ${res.status})` };
			}
			throw err;
		}
		return res;
	}

	async request<T>(path: string, options: RequestInit = {}): Promise<T> {
		const res = await this.send(path, options, true);
		if (res.status === 204) return undefined as T;
		const text = await res.text();
		return (text ? JSON.parse(text) : undefined) as T;
	}

	get<T>(path: string, params?: Params) {
		let url = path;
		if (params) {
			const q = new URLSearchParams();
			for (const [k, v] of Object.entries(params)) {
				if (v !== undefined && v !== null && v !== '') q.set(k, String(v));
			}
			const qs = q.toString();
			if (qs) url += `?${qs}`;
		}
		return this.request<T>(url, { method: 'GET' });
	}

	post<T>(path: string, body?: unknown) {
		return this.request<T>(path, { method: 'POST', body: body === undefined ? undefined : JSON.stringify(body) });
	}

	patch<T>(path: string, body?: unknown) {
		return this.request<T>(path, { method: 'PATCH', body: body === undefined ? undefined : JSON.stringify(body) });
	}

	put<T>(path: string, body?: unknown) {
		return this.request<T>(path, { method: 'PUT', body: body === undefined ? undefined : JSON.stringify(body) });
	}

	delete<T>(path: string) {
		return this.request<T>(path, { method: 'DELETE' });
	}

	uploadFile<T>(path: string, file: File, extraFields?: Record<string, string>) {
		const form = new FormData();
		form.append('file', file);
		for (const [k, v] of Object.entries(extraFields ?? {})) form.append(k, v);
		return this.request<T>(path, { method: 'POST', body: form });
	}

	/** Fetches a protected file (e.g. a PDF) with the auth header and opens it in a new tab. */
	async openFile(path: string, filename: string) {
		// Open the window synchronously so popup blockers allow it.
		const win = typeof window !== 'undefined' ? window.open('', '_blank') : null;
		try {
			const res = await this.send(path, { method: 'GET' }, true);
			const blob = await res.blob();
			const url = URL.createObjectURL(blob);
			if (win) {
				win.location.href = url;
			} else {
				const a = document.createElement('a');
				a.href = url;
				a.download = filename;
				a.click();
			}
			setTimeout(() => URL.revokeObjectURL(url), 60_000);
		} catch (e) {
			win?.close();
			throw e;
		}
	}
}

export const api = new ApiClient();

type Q = Params;

// ── Auth ──────────────────────────────────────────────────────
export const authApi = {
	login: (data: LoginRequest) => api.post<LoginResponse>('/api/v1/auth/login', data),
	logout: () => api.post<void>('/api/v1/auth/logout'),
	me: () => api.get<AuthUser>('/api/v1/auth/me'),
	changePassword: (current_password: string, new_password: string) =>
		api.post<{ message: string; refresh_token?: string }>('/api/v1/auth/change-password', { current_password, new_password })
};

// ── Dashboard ─────────────────────────────────────────────────
export const dashboardApi = {
	summary: () => api.get('/api/v1/dashboard/summary'),
	salesTrend: (params?: { days?: number }) => api.get('/api/v1/dashboard/sales', params),
	activity: () => api.get('/api/v1/dashboard/activity'),
	topProducts: () => api.get('/api/v1/dashboard/top-products')
};

// ── Inventory ─────────────────────────────────────────────────
export const inventoryApi = {
	products: {
		list: (p?: Q) => api.get('/api/v1/inventory/products', p),
		get: (id: string) => api.get(`/api/v1/inventory/products/${id}`),
		create: (data: unknown) => api.post('/api/v1/inventory/products', data),
		update: (id: string, data: unknown) => api.patch(`/api/v1/inventory/products/${id}`, data)
	},
	adjust: (data: unknown) => api.post('/api/v1/inventory/adjustments', data),
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
		list: (p?: Q) => api.get('/api/v1/inventory/imports', p),
		get: (id: string) => api.get(`/api/v1/inventory/imports/${id}`),
		upload: (file: File, mode: string, locationId?: string) =>
			api.uploadFile('/api/v1/inventory/imports', file, locationId ? { mode, location_id: locationId } : { mode })
	},
	movements: (p?: Q) => api.get('/api/v1/inventory/movements', p),
	uploadImage: (file: File, productId?: string) =>
		api.uploadFile('/api/v1/uploads/images', file, productId ? { product_id: productId } : {})
};

// ── Racks ─────────────────────────────────────────────────────
export const rackApi = {
	warehouses: () => api.get('/api/v1/warehouses'),
	createWarehouse: (data: unknown) => api.post('/api/v1/warehouses', data),
	racks: (p?: Q) => api.get('/api/v1/racks', p),
	createRack: (data: unknown) => api.post('/api/v1/racks', data),
	rackInventory: (rackId: string) => api.get(`/api/v1/racks/${rackId}/inventory`),
	transfer: (data: unknown) => api.post('/api/v1/racks/transfers', data),
	stockCount: (data: unknown) => api.post('/api/v1/inventory/stock-counts', data)
};

// ── Purchases ─────────────────────────────────────────────────
export const purchaseApi = {
	list: (p?: Q) => api.get('/api/v1/purchases', p),
	get: (id: string) => api.get(`/api/v1/purchases/${id}`),
	create: (data: unknown) => api.post('/api/v1/purchases', data),
	update: (id: string, data: unknown) => api.patch(`/api/v1/purchases/${id}`, data),
	approve: (id: string) => api.post(`/api/v1/purchases/${id}/approve`, {}),
	cancel: (id: string, reason: string) => api.post(`/api/v1/purchases/${id}/cancel`, { reason }),
	receive: (id: string, data: unknown) => api.post(`/api/v1/purchases/${id}/receipts`, data),
	recordPayment: (id: string, data: unknown) => api.post(`/api/v1/purchases/${id}/payments`, data)
};

// ── Suppliers ─────────────────────────────────────────────────
export const supplierApi = {
	list: (p?: Q) => api.get('/api/v1/suppliers', p),
	get: (id: string) => api.get(`/api/v1/suppliers/${id}`),
	create: (data: unknown) => api.post('/api/v1/suppliers', data),
	update: (id: string, data: unknown) => api.patch(`/api/v1/suppliers/${id}`, data),
	ledger: (id: string) => api.get(`/api/v1/suppliers/${id}/ledger`)
};

// ── Customers ─────────────────────────────────────────────────
export const customerApi = {
	list: (p?: Q) => api.get('/api/v1/customers', p),
	get: (id: string) => api.get(`/api/v1/customers/${id}`),
	create: (data: unknown) => api.post('/api/v1/customers', data),
	update: (id: string, data: unknown) => api.patch(`/api/v1/customers/${id}`, data),
	ledger: (id: string) => api.get(`/api/v1/customers/${id}/ledger`)
};

// ── Invoices ──────────────────────────────────────────────────
export const invoiceApi = {
	list: (p?: Q) => api.get('/api/v1/invoices', p),
	get: (id: string) => api.get(`/api/v1/invoices/${id}`),
	create: (data: unknown) => api.post('/api/v1/sales/invoices', data),
	cancel: (id: string, reason: string) => api.post(`/api/v1/invoices/${id}/cancel`, { reason }),
	recordPayment: (id: string, data: unknown) => api.post(`/api/v1/invoices/${id}/payments`, data),
	creditNote: (id: string, data: unknown) => api.post(`/api/v1/invoices/${id}/credit-notes`, data),
	upiDetails: (id: string) => api.get(`/api/v1/invoices/${id}/upi-details`),
	openPdf: (id: string, number = 'invoice') => api.openFile(`/api/v1/invoices/${id}/pdf`, `${number}.pdf`)
};

// ── Payments ──────────────────────────────────────────────────
export const paymentApi = {
	list: (p?: Q) => api.get('/api/v1/payments', p),
	get: (id: string) => api.get(`/api/v1/payments/${id}`),
	createRazorpayOrder: (data: unknown) => api.post('/api/v1/payments/razorpay/orders', data)
};

// ── Finance ───────────────────────────────────────────────────
export const financeApi = {
	summary: (p?: Q) => api.get('/api/v1/finance/summary', p),
	payables: () => api.get('/api/v1/finance/payables'),
	receivables: () => api.get('/api/v1/finance/receivables'),
	expenses: (p?: Q) => api.get('/api/v1/finance/expenses', p),
	createExpense: (data: unknown) => api.post('/api/v1/finance/expenses', data),
	updateExpense: (id: string, data: unknown) => api.put(`/api/v1/finance/expenses/${id}`, data),
	deleteExpense: (id: string) => api.delete(`/api/v1/finance/expenses/${id}`),
	reviewExpense: (id: string, decision: 'approve' | 'reject') =>
		api.post(`/api/v1/finance/expenses/${id}/${decision}`, {}),
	updateDueDate: (kind: 'invoice' | 'purchase', id: string, due_date: string) =>
		api.put(`/api/v1/finance/due-date/${kind}/${id}`, { due_date })
};

// ── GST ───────────────────────────────────────────────────────
export const gstApi = {
	verify: (gstin: string) => api.post('/api/v1/gst/verify', { gstin }),
	history: () => api.get('/api/v1/gst/verifications')
};

// ── Reports ───────────────────────────────────────────────────
export const reportsApi = {
	inventory: (p?: Q) => api.get('/api/v1/reports/inventory', p),
	sales: (p?: Q) => api.get('/api/v1/reports/sales', p),
	purchases: (p?: Q) => api.get('/api/v1/reports/purchases', p),
	payables: () => api.get('/api/v1/reports/payables'),
	receivables: () => api.get('/api/v1/reports/receivables'),
	gst: (p?: Q) => api.get('/api/v1/reports/gst', p)
};

// ── Notifications ─────────────────────────────────────────────
export const notificationApi = {
	list: () => api.get('/api/v1/notifications'),
	markRead: (id: string) => api.patch(`/api/v1/notifications/${id}/read`),
	markAllRead: () => api.post('/api/v1/notifications/read-all')
};

// ── Users ─────────────────────────────────────────────────────
export const usersApi = {
	list: (p?: Q) => api.get('/api/v1/users', p),
	get: (id: string) => api.get(`/api/v1/users/${id}`),
	create: (data: unknown) => api.post('/api/v1/users', data),
	update: (id: string, data: unknown) => api.patch(`/api/v1/users/${id}`, data),
	deactivate: (id: string) => api.post(`/api/v1/users/${id}/deactivate`),
	resetPassword: (id: string, password: string) => api.post(`/api/v1/users/${id}/reset-password`, { password }),
	permissions: (id: string) => api.get(`/api/v1/users/${id}/permissions`),
	updatePermissions: (id: string, permissions: string[]) => api.put(`/api/v1/users/${id}/permissions`, { permissions }),
	allPermissions: () => api.get('/api/v1/permissions')
};

// ── Settings ──────────────────────────────────────────────────
export const settingsApi = {
	company: () => api.get('/api/v1/settings/company'),
	updateCompany: (data: unknown) => api.put('/api/v1/settings/company', data)
};

// ── Audit ─────────────────────────────────────────────────────
export const auditApi = {
	list: (p?: Q) => api.get('/api/v1/audit-logs', p)
};
