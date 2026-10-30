<script lang="ts">
	import { onMount } from 'svelte';
	import { invoiceApi, errorMessage } from '$api/client';
	import { currentUser, hasPermission } from '$stores/auth';
	import { toasts } from '$stores/toasts';
	import { formatINR, formatDate, formatDateTime, humanize, debounce, toNumber, STATUS_BADGE } from '$utils/format';

	type Row = Record<string, any>;

	let invoices: Row[] = [];
	let total = 0;
	let page = 1;
	const perPage = 25;
	let loading = true;
	let search = '';
	let statusFilter = '';
	let dateFrom = '';
	let dateTo = '';

	let detail: Row | null = null;
	let detailLoading = false;
	let mode: 'view' | 'pay' | 'cancel' | 'credit' = 'view';
	let busy = false;
	let payForm = { amount: '', payment_method: 'cash', transaction_reference: '' };
	let cancelReason = '';
	let creditForm = { amount: '', reason: '' };

	$: canPay = hasPermission($currentUser, 'payments.record');
	$: canCancel = hasPermission($currentUser, 'invoices.cancel');
	$: totalPages = Math.max(1, Math.ceil(total / perPage));

	async function load() {
		loading = true;
		try {
			const res: any = await invoiceApi.list({
				search, status: statusFilter, from: dateFrom, to: dateTo, page, per_page: perPage
			});
			invoices = res.data ?? [];
			total = res.total ?? invoices.length;
		} catch (e) {
			toasts.error('Failed to load invoices', errorMessage(e));
		}
		loading = false;
	}

	const reload = debounce(() => { page = 1; load(); }, 300);

	async function openInvoice(id: string) {
		mode = 'view';
		detailLoading = true;
		detail = { id };
		try {
			detail = (await invoiceApi.get(id)) as Row;
		} catch (e) {
			toasts.error('Failed to load invoice', errorMessage(e));
			detail = null;
		}
		detailLoading = false;
	}

	function close() { detail = null; mode = 'view'; }

	function startPay() {
		payForm = { amount: String(toNumber(detail?.outstanding_amount)), payment_method: 'cash', transaction_reference: '' };
		mode = 'pay';
	}

	async function submitPayment() {
		if (!detail) return;
		busy = true;
		try {
			await invoiceApi.recordPayment(detail.id, payForm);
			toasts.success('Payment recorded');
			await Promise.all([openInvoice(detail.id), load()]);
		} catch (e) { toasts.error('Payment failed', errorMessage(e)); }
		busy = false;
	}

	async function submitCancel() {
		if (!detail || !cancelReason.trim()) { toasts.warning('Enter a reason'); return; }
		busy = true;
		try {
			await invoiceApi.cancel(detail.id, cancelReason.trim());
			toasts.success('Invoice cancelled', 'Stock has been returned to the racks');
			cancelReason = '';
			await Promise.all([openInvoice(detail.id), load()]);
		} catch (e) { toasts.error('Could not cancel', errorMessage(e)); }
		busy = false;
	}

	async function submitCredit() {
		if (!detail) return;
		busy = true;
		try {
			const res: any = await invoiceApi.creditNote(detail.id, creditForm);
			toasts.success('Credit note issued', toNumber(res.refund_due) > 0 ? `Refund due: ${formatINR(res.refund_due)}` : undefined);
			creditForm = { amount: '', reason: '' };
			await Promise.all([openInvoice(detail.id), load()]);
		} catch (e) { toasts.error('Could not issue credit note', errorMessage(e)); }
		busy = false;
	}

	async function pdf(inv: Row) {
		try { await invoiceApi.openPdf(inv.id, inv.invoice_number); }
		catch (e) { toasts.error('Could not open PDF', errorMessage(e)); }
	}

	function onKey(e: KeyboardEvent) { if (e.key === 'Escape' && detail) close(); }

	onMount(load);
</script>

<svelte:window on:keydown={onKey} />
<svelte:head><title>Invoices — RARA Enterprise ERP</title></svelte:head>

<div class="page-container">
	<div class="page-header flex items-center justify-between gap-4 flex-wrap">
		<div>
			<h1 class="page-title">Invoices</h1>
			<p class="page-subtitle">{total.toLocaleString('en-IN')} invoice{total !== 1 ? 's' : ''}</p>
		</div>
		<a href="/pos" class="btn btn-primary">+ New Invoice</a>
	</div>

	<div class="card mb-4">
		<div class="card-body grid grid-cols-1 md:grid-cols-4 gap-3" style="padding:1rem">
			<input class="input" placeholder="Search invoice #, customer, phone…" bind:value={search} on:input={reload} aria-label="Search invoices" />
			<select class="input" bind:value={statusFilter} on:change={reload} aria-label="Filter by status">
				<option value="">All statuses</option>
				<option value="issued">Issued (unpaid)</option>
				<option value="partially_paid">Partially paid</option>
				<option value="paid">Paid</option>
				<option value="overdue">Overdue</option>
				<option value="returned">Returned</option>
				<option value="cancelled">Cancelled</option>
			</select>
			<input class="input" type="date" bind:value={dateFrom} on:change={reload} aria-label="From date" />
			<input class="input" type="date" bind:value={dateTo} on:change={reload} aria-label="To date" />
		</div>
	</div>

	<div class="card">
		{#if loading}
			<div class="flex items-center justify-center gap-3 py-16 text-slate-400"><div class="spinner"></div> Loading…</div>
		{:else if invoices.length === 0}
			<div class="empty-state py-16">
				<h3>No invoices found</h3>
				<p>Create invoices from the POS / Billing screen.</p>
			</div>
		{:else}
			<div class="table-container">
				<table class="table">
					<thead><tr>
						<th>Invoice #</th><th>Date</th><th>Customer</th>
						<th class="text-right">Amount</th><th class="text-right">Paid</th><th class="text-right">Balance</th>
						<th>Status</th><th class="text-right">Actions</th>
					</tr></thead>
					<tbody>
					{#each invoices as inv (inv.id)}
						<tr class="clickable" on:click={() => openInvoice(inv.id)}>
							<td><span class="font-mono text-sm font-semibold text-yellow-700">{inv.invoice_number}</span></td>
							<td class="text-slate-600 text-sm">{formatDate(inv.invoice_date)}</td>
							<td>{inv.customer_name ?? 'Walk-in'}</td>
							<td class="text-right font-semibold">{formatINR(inv.grand_total)}</td>
							<td class="text-right text-green-700">{formatINR(inv.amount_paid)}</td>
							<td class="text-right font-semibold {toNumber(inv.outstanding_amount) > 0 ? 'text-red-600' : 'text-slate-400'}">{formatINR(inv.outstanding_amount)}</td>
							<td><span class="badge {STATUS_BADGE[inv.status] ?? 'badge-secondary'}">{humanize(inv.status)}</span></td>
							<td class="text-right">
								<button class="btn btn-ghost btn-sm" on:click|stopPropagation={() => pdf(inv)} title="Open PDF">PDF</button>
							</td>
						</tr>
					{/each}
					</tbody>
				</table>
			</div>
			{#if totalPages > 1}
				<div class="pagination">
					<span>Page {page} of {totalPages}</span>
					<div class="flex gap-2">
						<button class="btn btn-secondary btn-sm" disabled={page <= 1} on:click={() => { page--; load(); }}>Previous</button>
						<button class="btn btn-secondary btn-sm" disabled={page >= totalPages} on:click={() => { page++; load(); }}>Next</button>
					</div>
				</div>
			{/if}
		{/if}
	</div>
</div>

{#if detail}
	<div class="drawer-overlay" on:click={close} role="presentation"></div>
	<div class="drawer" role="dialog" tabindex="-1" aria-modal="true" aria-label="Invoice details">
		<div class="drawer-header">
			<div>
				<p class="text-xs uppercase tracking-wide text-slate-400 font-semibold">Invoice</p>
				<h2 class="text-xl font-bold font-mono">{detail.invoice_number ?? '…'}</h2>
				{#if detail.status}
					<span class="badge {STATUS_BADGE[detail.status] ?? 'badge-secondary'} mt-1">{humanize(detail.status)}</span>
				{/if}
			</div>
			<button class="modal-close" on:click={close} aria-label="Close">×</button>
		</div>

		<div class="drawer-body">
			{#if detailLoading}
				<div class="flex items-center justify-center py-16"><div class="spinner"></div></div>
			{:else}
				<dl class="kv mb-6">
					<div><dt>Customer</dt><dd>{detail.customer_name ?? 'Walk-in customer'}</dd></div>
					{#if detail.customer_phone}<div><dt>Phone</dt><dd>{detail.customer_phone}</dd></div>{/if}
					{#if detail.customer_gstin}<div><dt>GSTIN</dt><dd class="font-mono">{detail.customer_gstin}</dd></div>{/if}
					<div><dt>Date</dt><dd>{formatDate(detail.invoice_date)}</dd></div>
					{#if detail.due_date}<div><dt>Due</dt><dd>{formatDate(detail.due_date)}</dd></div>{/if}
					{#if detail.created_by_name}<div><dt>Billed by</dt><dd>{detail.created_by_name}</dd></div>{/if}
				</dl>

				<div class="table-container border border-slate-100 rounded-xl mb-4">
					<table class="table">
						<thead><tr><th>Item</th><th class="text-right">Qty</th><th class="text-right">Rate</th><th class="text-right">GST</th><th class="text-right">Total</th></tr></thead>
						<tbody>
						{#each detail.items ?? [] as it}
							<tr>
								<td><div class="font-medium">{it.product_name}</div><div class="text-xs text-slate-400 font-mono">{it.sku}{toNumber(it.discount_percent) > 0 ? ` · ${toNumber(it.discount_percent)}% off` : ''}</div></td>
								<td class="text-right">{toNumber(it.quantity)}</td>
								<td class="text-right">{formatINR(it.unit_price)}</td>
								<td class="text-right text-sm">{toNumber(it.gst_rate)}%</td>
								<td class="text-right font-semibold">{formatINR(it.total)}</td>
							</tr>
						{/each}
						</tbody>
					</table>
				</div>

				<div class="ml-auto max-w-xs space-y-1 text-sm mb-6">
					<div class="flex justify-between"><span class="text-slate-500">Taxable value</span><span>{formatINR(detail.taxable_value)}</span></div>
					{#if toNumber(detail.discount_amount) > 0}<div class="flex justify-between"><span class="text-slate-500">Discount</span><span>−{formatINR(detail.discount_amount)}</span></div>{/if}
					{#if toNumber(detail.igst_amount) > 0}
						<div class="flex justify-between"><span class="text-slate-500">IGST</span><span>{formatINR(detail.igst_amount)}</span></div>
					{:else}
						<div class="flex justify-between"><span class="text-slate-500">CGST</span><span>{formatINR(detail.cgst_amount)}</span></div>
						<div class="flex justify-between"><span class="text-slate-500">SGST</span><span>{formatINR(detail.sgst_amount)}</span></div>
					{/if}
					<div class="flex justify-between text-base font-bold border-t border-slate-200 pt-2"><span>Total</span><span>{formatINR(detail.grand_total)}</span></div>
					<div class="flex justify-between text-green-700"><span>Paid</span><span>{formatINR(detail.amount_paid)}</span></div>
					<div class="flex justify-between font-semibold {toNumber(detail.outstanding_amount) > 0 ? 'text-red-600' : 'text-slate-400'}"><span>Balance due</span><span>{formatINR(detail.outstanding_amount)}</span></div>
				</div>

				{#if (detail.payments ?? []).length}
					<h3 class="font-semibold text-slate-700 mb-2">Payments</h3>
					<ul class="divide-y divide-slate-100 mb-6 text-sm">
						{#each detail.payments as p}
							<li class="py-2 flex justify-between">
								<span>{formatDateTime(p.created_at)} · {humanize(p.payment_method)}{p.transaction_reference ? ` · ${p.transaction_reference}` : ''}</span>
								<span class="font-semibold text-green-700">{formatINR(p.amount)}</span>
							</li>
						{/each}
					</ul>
				{/if}

				{#if (detail.credit_notes ?? []).length}
					<h3 class="font-semibold text-slate-700 mb-2">Credit notes</h3>
					<ul class="divide-y divide-slate-100 mb-6 text-sm">
						{#each detail.credit_notes as c}
							<li class="py-2 flex justify-between"><span>{formatDate(c.created_at)} · {c.reason}</span><span class="font-semibold">{formatINR(c.total)}</span></li>
						{/each}
					</ul>
				{/if}

				{#if detail.notes}<p class="text-sm text-slate-500 whitespace-pre-line mb-4">{detail.notes}</p>{/if}

				{#if mode === 'pay'}
					<form class="card card-body space-y-3" on:submit|preventDefault={submitPayment}>
						<h3 class="font-semibold">Record payment</h3>
						<div class="grid grid-cols-2 gap-3">
							<div class="form-group"><label class="form-label" for="pay-amt">Amount</label>
								<input id="pay-amt" class="input" type="number" step="0.01" min="0.01" max={toNumber(detail.outstanding_amount)} bind:value={payForm.amount} required /></div>
							<div class="form-group"><label class="form-label" for="pay-method">Method</label>
								<select id="pay-method" class="input" bind:value={payForm.payment_method}>
									<option value="cash">Cash</option><option value="upi">UPI</option>
									<option value="bank_transfer">Bank transfer</option><option value="cheque">Cheque</option>
								</select></div>
						</div>
						<div class="form-group"><label class="form-label" for="pay-ref">Reference (UTR / cheque no.)</label>
							<input id="pay-ref" class="input" bind:value={payForm.transaction_reference} /></div>
						<div class="flex justify-end gap-2">
							<button type="button" class="btn btn-secondary" on:click={() => mode = 'view'}>Back</button>
							<button type="submit" class="btn btn-primary" disabled={busy}>{busy ? 'Saving…' : 'Save payment'}</button>
						</div>
					</form>
				{:else if mode === 'cancel'}
					<form class="card card-body space-y-3" on:submit|preventDefault={submitCancel}>
						<h3 class="font-semibold text-red-700">Cancel invoice</h3>
						<p class="text-sm text-slate-500">Stock will be returned to the racks it was sold from and the customer's balance reduced.</p>
						<div class="form-group"><label class="form-label required" for="cancel-reason">Reason</label>
							<input id="cancel-reason" class="input" bind:value={cancelReason} required /></div>
						<div class="flex justify-end gap-2">
							<button type="button" class="btn btn-secondary" on:click={() => mode = 'view'}>Back</button>
							<button type="submit" class="btn btn-danger" disabled={busy}>{busy ? 'Cancelling…' : 'Cancel invoice'}</button>
						</div>
					</form>
				{:else if mode === 'credit'}
					<form class="card card-body space-y-3" on:submit|preventDefault={submitCredit}>
						<h3 class="font-semibold">Issue credit note</h3>
						<p class="text-sm text-slate-500">Reduces the balance due. Any amount above the balance is shown as a refund due.</p>
						<div class="grid grid-cols-2 gap-3">
							<div class="form-group"><label class="form-label required" for="cn-amt">Amount</label>
								<input id="cn-amt" class="input" type="number" step="0.01" min="0.01" bind:value={creditForm.amount} required /></div>
							<div class="form-group"><label class="form-label" for="cn-reason">Reason</label>
								<input id="cn-reason" class="input" bind:value={creditForm.reason} placeholder="Return / rate difference" /></div>
						</div>
						<div class="flex justify-end gap-2">
							<button type="button" class="btn btn-secondary" on:click={() => mode = 'view'}>Back</button>
							<button type="submit" class="btn btn-primary" disabled={busy}>{busy ? 'Saving…' : 'Issue credit note'}</button>
						</div>
					</form>
				{/if}
			{/if}
		</div>

		{#if !detailLoading && detail.status && mode === 'view'}
			<div class="drawer-footer">
				<button class="btn btn-secondary" on:click={() => detail && pdf(detail)}>Print / PDF</button>
				{#if detail.status !== 'cancelled'}
					{#if canCancel && toNumber(detail.amount_paid) === 0}
						<button class="btn btn-danger" on:click={() => mode = 'cancel'}>Cancel invoice</button>
					{/if}
					{#if canCancel && detail.status !== 'returned'}
						<button class="btn btn-secondary" on:click={() => mode = 'credit'}>Credit note</button>
					{/if}
					{#if canPay && toNumber(detail.outstanding_amount) > 0}
						<button class="btn btn-primary" on:click={startPay}>Record payment</button>
					{/if}
				{/if}
			</div>
		{/if}
	</div>
{/if}
