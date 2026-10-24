<script lang="ts">
	import { onMount } from 'svelte';
	import { purchaseApi, supplierApi } from '$api/client';
	import { toasts } from '$stores/toasts';

	let purchases: Record<string, unknown>[] = [];
	let suppliers: Record<string, unknown>[] = [];
	let total = 0;
	let loading = true;
	let search = '';
	let statusFilter = '';
	let paymentFilter = '';
	let showModal = false;
	let submitting = false;
	let page = 1;

	const newPO = {
		supplier_id: '', supplier_invoice_number: '',
		notes: '', payment_terms_days: '30', freight: '0',
		items: [{ product_id: '', quantity: '1', unit_cost: '0', discount_percent: '0', gst_rate: '18' }]
	};

	let products: Record<string, unknown>[] = [];

	async function loadProducts() {
		try {
			const res = await (await import('$api/client')).inventoryApi.products.list({ per_page: 500 }) as { data: Record<string, unknown>[] };
			products = res.data ?? [];
		} catch {}
	}

	const formatINR = (v: unknown) => new Intl.NumberFormat('en-IN', { style: 'currency', currency: 'INR', maximumFractionDigits: 2 }).format(Number(v ?? 0));
	const formatDate = (d: unknown) => d ? new Date(String(d)).toLocaleDateString('en-IN') : '—';

	const STATUS_COLORS: Record<string, string> = {
		draft: 'badge-warning', approved: 'badge-info', ordered: 'badge-primary',
		partially_received: 'badge-warning', received: 'badge-success', cancelled: 'badge-danger'
	};

	const PAYMENT_COLORS: Record<string, string> = {
		unpaid: 'badge-danger', partially_paid: 'badge-warning', paid: 'badge-success', overdue: 'badge-danger'
	};

	async function loadData() {
		loading = true;
		try {
			const [res, supRes] = await Promise.all([
				purchaseApi.list({ page, per_page: 20, status: statusFilter || undefined }) as Promise<{data: Record<string, unknown>[]; total: number}>,
				supplierApi.list({ per_page: 100 }) as Promise<{data: Record<string, unknown>[]}>
			]);
			purchases = res.data ?? [];
			total = res.total ?? 0;
			suppliers = supRes.data ?? [];
		} catch (e: unknown) {
			toasts.error('Failed to load purchases', (e as {message?: string}).message);
		} finally {
			loading = false;
		}
	}

	async function createPO() {
		// Validate required fields only
		if (!newPO.supplier_id) { toasts.error('Select a supplier'); return; }

		submitting = true;
		try {
			// Sanitize payload for Rust backend
			const payload = {
				supplier_id: newPO.supplier_id,
				supplier_invoice_number: newPO.supplier_invoice_number || null,
				notes: newPO.notes || null,
				payment_terms_days: Number(newPO.payment_terms_days) || 30,
				freight: Number(newPO.freight) || 0,
				items: [] // Items added after creation
			};
			const res = await purchaseApi.create(payload) as { po_number: string };
			toasts.success(`Purchase Order ${res.po_number} created!`);
			showModal = false;
			newPO.supplier_id = ''; newPO.supplier_invoice_number = '';
			newPO.notes = ''; newPO.payment_terms_days = '30'; newPO.freight = '0';
			loadData();
		} catch (e: unknown) {
			toasts.error('Failed to create PO: ' + ((e as {message?: string}).message ?? 'Unknown error'));
		} finally {
			submitting = false;
		}
	}

	onMount(() => { loadData(); loadProducts(); });
</script>

<svelte:head><title>Purchases — RARA Enterprise ERP</title></svelte:head>

<div class="page-container">
	<div class="page-header">
		<div>
			<h1 class="page-title">Purchase Orders</h1>
			<p class="page-subtitle">{total.toLocaleString('en-IN')} total orders</p>
		</div>
		<button class="btn btn-primary" on:click={() => showModal = true}>
			<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
			New Purchase Order
		</button>
	</div>

	<!-- Filters -->
	<div class="card mb-5">
		<div class="card-body">
			<div class="grid grid-cols-1 md:grid-cols-3 gap-3">
				<select class="form-control" bind:value={statusFilter} on:change={loadData}>
					<option value="">All Statuses</option>
					<option value="draft">Draft</option>
					<option value="approved">Approved</option>
					<option value="ordered">Ordered</option>
					<option value="partially_received">Partially Received</option>
					<option value="received">Received</option>
					<option value="cancelled">Cancelled</option>
				</select>
				<select class="form-control" bind:value={paymentFilter} on:change={loadData}>
					<option value="">All Payment Statuses</option>
					<option value="unpaid">Unpaid</option>
					<option value="partially_paid">Partially Paid</option>
					<option value="paid">Paid</option>
					<option value="overdue">Overdue</option>
				</select>
				<input type="text" class="form-control" placeholder="Search PO number…" bind:value={search} />
			</div>
		</div>
	</div>

	<!-- Table -->
	<div class="card">
		{#if loading}
			<div class="p-10 flex items-center justify-center gap-3 text-slate-500">
				<div class="spinner"></div> Loading purchases…
			</div>
		{:else if purchases.length === 0}
			<div class="empty-state py-16">
				<div class="empty-state-icon">
					<svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="1" y="3" width="15" height="13"/><polygon points="16 8 20 8 23 11 23 16 16 16 16 8"/><circle cx="5.5" cy="18.5" r="2.5"/><circle cx="18.5" cy="18.5" r="2.5"/></svg>
				</div>
				<h3>No purchase orders</h3>
				<p>Create your first purchase order to get started.</p>
				<button class="btn btn-primary mt-4" on:click={() => showModal = true}>New Purchase Order</button>
			</div>
		{:else}
			<div class="table-container">
				<table class="data-table">
					<thead>
						<tr>
							<th>PO Number</th>
							<th>Supplier</th>
							<th>Date</th>
							<th class="text-right">Total</th>
							<th class="text-right">Outstanding</th>
							<th>Status</th>
							<th>Payment</th>
							<th class="text-right">Actions</th>
						</tr>
					</thead>
					<tbody>
						{#each purchases as po}
							{@const p = po as {id:string;po_number:string;supplier_name:string;purchase_date:string;grand_total:unknown;outstanding_amount:unknown;status:string;payment_status:string;due_date:string}}
							<tr>
								<td>
									<a href="/purchases/{p.id}" class="font-mono text-sm font-semibold text-yellow-700 hover:text-yellow-900">{p.po_number}</a>
								</td>
								<td class="text-sm text-slate-700">{p.supplier_name ?? '—'}</td>
								<td class="text-sm text-slate-500">{formatDate(p.purchase_date)}</td>
								<td class="text-right font-mono font-semibold text-sm">{formatINR(p.grand_total)}</td>
								<td class="text-right font-mono text-sm {Number(p.outstanding_amount) > 0 ? 'text-red-600 font-semibold' : 'text-green-600'}">{formatINR(p.outstanding_amount)}</td>
								<td><span class="badge {STATUS_COLORS[p.status] ?? 'badge-warning'}">{p.status.replace('_', ' ')}</span></td>
								<td><span class="badge {PAYMENT_COLORS[p.payment_status] ?? 'badge-warning'}">{p.payment_status.replace('_', ' ')}</span></td>
								<td class="text-right">
									<a href="/purchases/{p.id}" class="btn btn-ghost btn-sm" title="View details">
										<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/><circle cx="12" cy="12" r="3"/></svg>
									</a>
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{/if}
	</div>
</div>

<!-- New PO Modal -->
{#if showModal}
	<!-- svelte-ignore a11y_interactive_supports_focus -->
	<div class="modal-overlay"
		on:click|self={() => showModal = false}
		on:keydown={(e) => e.key === 'Escape' && (showModal = false)}
		role="dialog" aria-label="New Purchase Order" aria-modal="true">
		<div class="modal" style="max-width: 560px">
			<div class="modal-header">
				<h2 class="modal-title">New Purchase Order</h2>
				<button class="btn btn-ghost btn-sm" on:click={() => showModal = false} aria-label="Close">
					<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
				</button>
			</div>
			<div class="modal-body">
				<form on:submit|preventDefault={createPO}>
					<div class="grid grid-cols-2 gap-4">
						<div class="form-group col-span-2">
							<label class="form-label required" for="po-supplier">Supplier</label>
							<select id="po-supplier" class="form-control" bind:value={newPO.supplier_id} required>
								<option value="">Select supplier</option>
								{#each suppliers as s}
									<option value={s.id}>{s.business_name}</option>
								{/each}
							</select>
						</div>
						<div class="form-group">
							<label class="form-label" for="po-inv">Supplier Invoice No.</label>
							<input id="po-inv" type="text" class="form-control" bind:value={newPO.supplier_invoice_number} placeholder="Optional" />
						</div>
						<div class="form-group">
							<label class="form-label" for="po-terms">Payment Terms (days)</label>
							<input id="po-terms" type="number" class="form-control" bind:value={newPO.payment_terms_days} min="0" />
						</div>
						<div class="form-group">
							<label class="form-label" for="po-freight">Freight (₹)</label>
							<input id="po-freight" type="number" step="0.01" class="form-control" bind:value={newPO.freight} min="0" />
						</div>
						<div class="form-group col-span-2">
							<label class="form-label" for="po-notes">Notes</label>
							<textarea id="po-notes" class="form-control" rows="2" bind:value={newPO.notes}></textarea>
						</div>
					</div>
					<p class="text-xs text-slate-500 mt-2">Items can be added after creating the PO.</p>
					<div class="modal-footer">
						<button type="button" class="btn btn-secondary" on:click={() => showModal = false}>Cancel</button>
						<button type="submit" class="btn btn-primary" disabled={submitting || !newPO.supplier_id}>
							{submitting ? 'Creating…' : 'Create PO'}
						</button>
					</div>
				</form>
			</div>
		</div>
	</div>
{/if}
