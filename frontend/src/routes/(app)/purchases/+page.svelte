<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { purchaseApi, supplierApi, inventoryApi, errorMessage } from '$api/client';
	import { currentUser, hasPermission } from '$stores/auth';
	import { toasts } from '$stores/toasts';
	import { formatINR, formatDate, humanize, debounce, toNumber, STATUS_BADGE } from '$utils/format';

	type Row = Record<string, any>;
	type Line = { product_id: string; quantity: string; unit_cost: string; discount_percent: string; gst_rate: string };

	let purchases: Row[] = [];
	let suppliers: Row[] = [];
	let products: Row[] = [];
	let total = 0;
	let loading = true;
	let search = '';
	let statusFilter = '';
	let paymentFilter = '';
	let page = 1;
	const perPage = 25;

	let showModal = false;
	let submitting = false;
	const blankLine = (): Line => ({ product_id: '', quantity: '1', unit_cost: '', discount_percent: '0', gst_rate: '18' });
	let form = {
		supplier_id: '', supplier_invoice_number: '', notes: '', payment_terms_days: '30',
		freight: '0', other_charges: '0', advance_paid: '0', payment_method: 'cash',
		items: [blankLine()]
	};

	$: canCreate = hasPermission($currentUser, 'purchases.create');
	$: totalPages = Math.max(1, Math.ceil(total / perPage));

	function lineTotals(l: Line) {
		const gross = toNumber(l.quantity) * toNumber(l.unit_cost);
		const taxable = gross * (1 - toNumber(l.discount_percent) / 100);
		const gst = taxable * toNumber(l.gst_rate) / 100;
		return { taxable, gst, total: taxable + gst };
	}
	$: totals = form.items.reduce(
		(acc, l) => { const t = lineTotals(l); return { taxable: acc.taxable + t.taxable, gst: acc.gst + t.gst }; },
		{ taxable: 0, gst: 0 }
	);
	$: grandTotal = totals.taxable + totals.gst + toNumber(form.freight) + toNumber(form.other_charges);

	async function loadData() {
		loading = true;
		try {
			const res: any = await purchaseApi.list({
				page, per_page: perPage, status: statusFilter, payment_status: paymentFilter, search
			});
			purchases = res.data ?? [];
			total = res.total ?? 0;
		} catch (e) {
			toasts.error('Failed to load purchases', errorMessage(e));
		}
		loading = false;
	}
	const reload = debounce(() => { page = 1; loadData(); }, 300);

	async function openModal() {
		showModal = true;
		if (suppliers.length && products.length) return;
		try {
			const [s, p]: any[] = await Promise.all([
				supplierApi.list({ status: 'active', per_page: 500 }),
				inventoryApi.products.list({ status: 'active', per_page: 500 })
			]);
			suppliers = s.data ?? [];
			products = p.data ?? [];
		} catch (e) { toasts.error('Failed to load suppliers/products', errorMessage(e)); }
	}

	function onSupplierChange() {
		const s = suppliers.find((x) => x.id === form.supplier_id);
		if (s) form.payment_terms_days = String(s.payment_terms_days ?? 30);
	}

	function onProductChange(i: number) {
		const p = products.find((x) => x.id === form.items[i].product_id);
		if (p) {
			form.items[i].unit_cost = String(toNumber(p.purchase_price) || '');
			form.items[i].gst_rate = String(toNumber(p.gst_rate));
		}
		form = form;
	}

	function addLine() { form.items = [...form.items, blankLine()]; }
	function removeLine(i: number) { form.items = form.items.filter((_, idx) => idx !== i); if (!form.items.length) addLine(); }

	async function createPO() {
		const items = form.items.filter((l) => l.product_id);
		if (!form.supplier_id) { toasts.error('Select a supplier'); return; }
		if (!items.length) { toasts.error('Add at least one product'); return; }
		submitting = true;
		try {
			const res: any = await purchaseApi.create({ ...form, items });
			toasts.success(`Purchase order ${res.po_number} created`, `Total ${formatINR(res.grand_total)}`);
			showModal = false;
			form = { ...form, supplier_id: '', supplier_invoice_number: '', notes: '', freight: '0', other_charges: '0', advance_paid: '0', items: [blankLine()] };
			goto(`/purchases/${res.id}`);
		} catch (e) {
			toasts.error('Failed to create purchase order', errorMessage(e));
		}
		submitting = false;
	}

	onMount(loadData);
</script>

<svelte:head><title>Purchases — RARA Enterprise ERP</title></svelte:head>

<div class="page-container">
	<div class="page-header flex items-center justify-between gap-4 flex-wrap">
		<div>
			<h1 class="page-title">Purchase Orders</h1>
			<p class="page-subtitle">{total.toLocaleString('en-IN')} orders</p>
		</div>
		{#if canCreate}
			<button class="btn btn-primary" on:click={openModal}>+ New Purchase Order</button>
		{/if}
	</div>

	<div class="card mb-5">
		<div class="card-body grid grid-cols-1 md:grid-cols-3 gap-3">
			<input type="text" class="form-control" placeholder="Search PO #, supplier, supplier invoice…" bind:value={search} on:input={reload} aria-label="Search purchases" />
			<select class="form-control" bind:value={statusFilter} on:change={reload} aria-label="Filter by status">
				<option value="">All statuses</option>
				<option value="draft">Draft</option>
				<option value="approved">Approved</option>
				<option value="partially_received">Partially received</option>
				<option value="received">Received</option>
				<option value="cancelled">Cancelled</option>
			</select>
			<select class="form-control" bind:value={paymentFilter} on:change={reload} aria-label="Filter by payment status">
				<option value="">All payment statuses</option>
				<option value="unpaid">Unpaid</option>
				<option value="partially_paid">Partially paid</option>
				<option value="paid">Paid</option>
				<option value="overdue">Overdue</option>
			</select>
		</div>
	</div>

	<div class="card">
		{#if loading}
			<div class="p-10 flex items-center justify-center gap-3 text-slate-500"><div class="spinner"></div> Loading purchases…</div>
		{:else if purchases.length === 0}
			<div class="empty-state py-16">
				<h3>No purchase orders</h3>
				<p>Create a purchase order to restock from a supplier.</p>
				{#if canCreate}<button class="btn btn-primary mt-4" on:click={openModal}>New Purchase Order</button>{/if}
			</div>
		{:else}
			<div class="table-container">
				<table class="data-table">
					<thead>
						<tr>
							<th>PO Number</th><th>Supplier</th><th>Date</th>
							<th class="text-right">Total</th><th class="text-right">Outstanding</th>
							<th>Status</th><th>Payment</th><th>Due</th>
						</tr>
					</thead>
					<tbody>
						{#each purchases as p (p.id)}
							<tr class="clickable" on:click={() => goto(`/purchases/${p.id}`)}>
								<td><a href="/purchases/{p.id}" class="font-mono text-sm font-semibold text-yellow-700 hover:text-yellow-900" on:click|stopPropagation>{p.po_number}</a></td>
								<td class="text-sm text-slate-700">{p.supplier_name ?? '—'}</td>
								<td class="text-sm text-slate-500">{formatDate(p.purchase_date)}</td>
								<td class="text-right font-mono font-semibold text-sm">{formatINR(p.grand_total)}</td>
								<td class="text-right font-mono text-sm {toNumber(p.outstanding_amount) > 0 ? 'text-red-600 font-semibold' : 'text-green-600'}">{formatINR(p.outstanding_amount)}</td>
								<td><span class="badge {STATUS_BADGE[p.status] ?? 'badge-warning'}">{humanize(p.status)}</span></td>
								<td><span class="badge {STATUS_BADGE[p.payment_status] ?? 'badge-warning'}">{humanize(p.payment_status)}</span></td>
								<td class="text-sm text-slate-500">{formatDate(p.due_date)}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
			{#if totalPages > 1}
				<div class="pagination">
					<span>Page {page} of {totalPages}</span>
					<div class="flex gap-2">
						<button class="btn btn-secondary btn-sm" disabled={page <= 1} on:click={() => { page--; loadData(); }}>Previous</button>
						<button class="btn btn-secondary btn-sm" disabled={page >= totalPages} on:click={() => { page++; loadData(); }}>Next</button>
					</div>
				</div>
			{/if}
		{/if}
	</div>
</div>

{#if showModal}
	<div class="modal-overlay" on:click|self={() => (showModal = false)} on:keydown={(e) => e.key === 'Escape' && (showModal = false)}
		role="dialog" aria-label="New purchase order" aria-modal="true" tabindex="-1">
		<div class="modal modal-xl">
			<div class="modal-header">
				<h2 class="modal-title">New Purchase Order</h2>
				<button class="modal-close" on:click={() => (showModal = false)} aria-label="Close">×</button>
			</div>
			<form class="modal-body space-y-5" on:submit|preventDefault={createPO}>
				<div class="grid grid-cols-1 md:grid-cols-4 gap-4">
					<div class="form-group md:col-span-2">
						<label class="form-label required" for="po-supplier">Supplier</label>
						<select id="po-supplier" class="form-control" bind:value={form.supplier_id} on:change={onSupplierChange} required>
							<option value="">Select supplier</option>
							{#each suppliers as s}<option value={s.id}>{s.business_name}</option>{/each}
						</select>
						{#if suppliers.length === 0}<span class="form-hint">No suppliers yet — <a class="underline" href="/suppliers">add one</a>.</span>{/if}
					</div>
					<div class="form-group">
						<label class="form-label" for="po-inv">Supplier invoice no.</label>
						<input id="po-inv" type="text" class="form-control" bind:value={form.supplier_invoice_number} placeholder="Optional" />
					</div>
					<div class="form-group">
						<label class="form-label" for="po-terms">Payment terms (days)</label>
						<input id="po-terms" type="number" class="form-control" bind:value={form.payment_terms_days} min="0" />
					</div>
				</div>

				<div class="table-container border border-slate-100 rounded-xl">
					<table class="table">
						<thead><tr><th style="min-width:260px">Product</th><th>Qty</th><th>Unit cost</th><th>Disc %</th><th>GST %</th><th class="text-right">Line total</th><th></th></tr></thead>
						<tbody>
							{#each form.items as line, i}
								<tr>
									<td>
										<select class="form-control" bind:value={line.product_id} on:change={() => onProductChange(i)} aria-label="Product for line {i + 1}">
											<option value="">Select product</option>
											{#each products as p}<option value={p.id}>{p.name} ({p.sku})</option>{/each}
										</select>
									</td>
									<td><input class="form-control" type="number" min="0.001" step="any" bind:value={line.quantity} style="width:90px" aria-label="Quantity" /></td>
									<td><input class="form-control" type="number" min="0" step="0.01" bind:value={line.unit_cost} style="width:110px" aria-label="Unit cost" /></td>
									<td><input class="form-control" type="number" min="0" max="100" step="0.01" bind:value={line.discount_percent} style="width:80px" aria-label="Discount percent" /></td>
									<td>
										<select class="form-control" bind:value={line.gst_rate} style="width:90px" aria-label="GST rate">
											{#each ['0', '5', '12', '18', '28'] as r}<option value={r}>{r}%</option>{/each}
										</select>
									</td>
									<td class="text-right font-mono">{formatINR(lineTotals(line).total)}</td>
									<td><button type="button" class="btn btn-ghost btn-sm" on:click={() => removeLine(i)} aria-label="Remove line">×</button></td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
				<button type="button" class="btn btn-secondary btn-sm" on:click={addLine}>+ Add line</button>

				<div class="grid grid-cols-1 md:grid-cols-2 gap-6">
					<div class="space-y-4">
						<div class="grid grid-cols-2 gap-4">
							<div class="form-group"><label class="form-label" for="po-freight">Freight (₹)</label>
								<input id="po-freight" type="number" step="0.01" min="0" class="form-control" bind:value={form.freight} /></div>
							<div class="form-group"><label class="form-label" for="po-other">Other charges (₹)</label>
								<input id="po-other" type="number" step="0.01" min="0" class="form-control" bind:value={form.other_charges} /></div>
							<div class="form-group"><label class="form-label" for="po-adv">Advance paid (₹)</label>
								<input id="po-adv" type="number" step="0.01" min="0" class="form-control" bind:value={form.advance_paid} /></div>
							<div class="form-group"><label class="form-label" for="po-adv-m">Advance method</label>
								<select id="po-adv-m" class="form-control" bind:value={form.payment_method}>
									<option value="cash">Cash</option><option value="bank_transfer">Bank transfer</option>
									<option value="upi">UPI</option><option value="cheque">Cheque</option>
								</select></div>
						</div>
						<div class="form-group"><label class="form-label" for="po-notes">Notes</label>
							<textarea id="po-notes" class="form-control" rows="2" bind:value={form.notes}></textarea></div>
					</div>
					<div class="bg-slate-50 rounded-xl p-4 space-y-2 text-sm self-start">
						<div class="flex justify-between"><span class="text-slate-500">Taxable value</span><span class="font-mono">{formatINR(totals.taxable)}</span></div>
						<div class="flex justify-between"><span class="text-slate-500">GST</span><span class="font-mono">{formatINR(totals.gst)}</span></div>
						<div class="flex justify-between"><span class="text-slate-500">Freight + other</span><span class="font-mono">{formatINR(toNumber(form.freight) + toNumber(form.other_charges))}</span></div>
						<div class="flex justify-between text-base font-bold border-t border-slate-200 pt-2"><span>Grand total</span><span class="font-mono">{formatINR(grandTotal)}</span></div>
						<div class="flex justify-between text-slate-600"><span>Balance payable</span><span class="font-mono">{formatINR(Math.max(0, grandTotal - toNumber(form.advance_paid)))}</span></div>
					</div>
				</div>

				<div class="modal-footer" style="padding:0;border:0">
					<button type="button" class="btn btn-secondary" on:click={() => (showModal = false)}>Cancel</button>
					<button type="submit" class="btn btn-primary" disabled={submitting || !form.supplier_id}>{submitting ? 'Creating…' : 'Create purchase order'}</button>
				</div>
			</form>
		</div>
	</div>
{/if}
