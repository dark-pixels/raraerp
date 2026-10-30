<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { purchaseApi, rackApi, errorMessage } from '$api/client';
	import { currentUser, hasPermission } from '$stores/auth';
	import { toasts } from '$stores/toasts';
	import { formatINR, formatDate, formatDateTime, humanize, toNumber, STATUS_BADGE } from '$utils/format';

	type Row = Record<string, any>;

	let po: Row | null = null;
	let racks: Row[] = [];
	let loading = true;
	let busy = false;
	let panel: '' | 'receive' | 'pay' | 'cancel' = '';

	let receiveQty: Record<string, string> = {};
	let locationId = '';
	let payForm = { amount: '', payment_method: 'bank_transfer', transaction_reference: '', notes: '' };
	let cancelReason = '';

	$: id = $page.params.id ?? '';
	$: canApprove = hasPermission($currentUser, 'purchases.approve');
	$: canReceive = hasPermission($currentUser, 'purchases.receive');
	$: canPay = hasPermission($currentUser, 'payments.record');
	$: receivable = po && ['approved', 'ordered', 'partially_received'].includes(po.status);
	$: cancellable = po && ['draft', 'approved', 'ordered'].includes(po.status) && toNumber(po.amount_paid) === 0;

	async function load() {
		loading = true;
		try {
			po = (await purchaseApi.get(id)) as Row;
		} catch (e) {
			toasts.error('Failed to load purchase order', errorMessage(e));
		}
		loading = false;
	}

	async function openReceive() {
		panel = 'receive';
		receiveQty = Object.fromEntries((po?.items ?? []).map((i: Row) => [i.product_id, String(toNumber(i.pending_quantity))]));
		if (!racks.length) {
			try { racks = ((await rackApi.racks()) as any).data ?? []; } catch { racks = []; }
		}
	}

	async function approve() {
		busy = true;
		try { await purchaseApi.approve(id); toasts.success('Purchase order approved'); await load(); }
		catch (e) { toasts.error('Could not approve', errorMessage(e)); }
		busy = false;
	}

	async function receive() {
		const items = Object.entries(receiveQty)
			.filter(([, q]) => toNumber(q) > 0)
			.map(([product_id, q]) => ({ product_id, received_quantity: q }));
		if (!items.length) { toasts.warning('Enter quantities to receive'); return; }
		busy = true;
		try {
			const res: any = await purchaseApi.receive(id, { items, location_id: locationId || undefined });
			toasts.success('Goods received', `Status: ${humanize(res.status)}`);
			panel = '';
			await load();
		} catch (e) { toasts.error('Could not receive goods', errorMessage(e)); }
		busy = false;
	}

	async function pay() {
		busy = true;
		try {
			await purchaseApi.recordPayment(id, payForm);
			toasts.success('Supplier payment recorded');
			panel = '';
			await load();
		} catch (e) { toasts.error('Payment failed', errorMessage(e)); }
		busy = false;
	}

	async function cancel() {
		if (!cancelReason.trim()) { toasts.warning('Enter a reason'); return; }
		busy = true;
		try { await purchaseApi.cancel(id, cancelReason.trim()); toasts.success('Purchase order cancelled'); panel = ''; await load(); }
		catch (e) { toasts.error('Could not cancel', errorMessage(e)); }
		busy = false;
	}

	onMount(load);
</script>

<svelte:head><title>{po?.po_number ?? 'Purchase Order'} — RARA Enterprise ERP</title></svelte:head>

<div class="page-container">
	<a href="/purchases" class="text-sm text-slate-500 hover:text-slate-800">← Purchase orders</a>

	{#if loading}
		<div class="flex items-center justify-center py-20"><div class="spinner"></div></div>
	{:else if !po}
		<div class="empty-state py-20"><h3>Purchase order not found</h3></div>
	{:else}
		<div class="page-header flex items-start justify-between gap-4 flex-wrap mt-2">
			<div>
				<h1 class="page-title font-mono">{po.po_number}</h1>
				<p class="page-subtitle">{po.supplier_name} · {formatDate(po.purchase_date)}</p>
				<div class="flex gap-2 mt-2">
					<span class="badge {STATUS_BADGE[po.status] ?? 'badge-secondary'}">{humanize(po.status)}</span>
					<span class="badge {STATUS_BADGE[po.payment_status] ?? 'badge-secondary'}">{humanize(po.payment_status)}</span>
				</div>
			</div>
			<div class="flex gap-2 flex-wrap no-print">
				<button class="btn btn-secondary" on:click={() => window.print()}>Print</button>
				{#if po.status === 'draft' && canApprove}
					<button class="btn btn-primary" on:click={approve} disabled={busy}>Approve</button>
				{/if}
				{#if receivable && canReceive}
					<button class="btn btn-primary" on:click={openReceive}>Receive goods</button>
				{/if}
				{#if po.status !== 'cancelled' && toNumber(po.outstanding_amount) > 0 && canPay}
					<button class="btn btn-secondary" on:click={() => { payForm.amount = String(toNumber(po?.outstanding_amount)); panel = 'pay'; }}>Pay supplier</button>
				{/if}
				{#if cancellable && canApprove}
					<button class="btn btn-danger" on:click={() => (panel = 'cancel')}>Cancel</button>
				{/if}
			</div>
		</div>

		<div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
			<div class="lg:col-span-2 space-y-6">
				{#if panel === 'receive'}
					<form class="card" on:submit|preventDefault={receive}>
						<div class="card-header"><h2 class="font-semibold">Receive goods</h2></div>
						<div class="card-body space-y-4">
							<div class="form-group">
								<label class="form-label" for="rcv-loc">Store in rack</label>
								<select id="rcv-loc" class="form-control" bind:value={locationId}>
									<option value="">Default location</option>
									{#each racks as r}<option value={r.id}>{r.label ?? r.rack_code}</option>{/each}
								</select>
							</div>
							<table class="table">
								<thead><tr><th>Product</th><th class="text-right">Ordered</th><th class="text-right">Received</th><th>Receive now</th></tr></thead>
								<tbody>
									{#each po.items as it}
										<tr>
											<td>{it.product_name}</td>
											<td class="text-right">{toNumber(it.quantity)}</td>
											<td class="text-right">{toNumber(it.received_quantity)}</td>
											<td>
												<input class="form-control" type="number" min="0" step="any" max={toNumber(it.pending_quantity)}
													bind:value={receiveQty[it.product_id]} disabled={toNumber(it.pending_quantity) <= 0}
													style="width:110px" aria-label="Quantity to receive for {it.product_name}" />
											</td>
										</tr>
									{/each}
								</tbody>
							</table>
							<div class="flex justify-end gap-2">
								<button type="button" class="btn btn-secondary" on:click={() => (panel = '')}>Close</button>
								<button type="submit" class="btn btn-primary" disabled={busy}>{busy ? 'Saving…' : 'Add to stock'}</button>
							</div>
						</div>
					</form>
				{:else if panel === 'pay'}
					<form class="card" on:submit|preventDefault={pay}>
						<div class="card-header"><h2 class="font-semibold">Pay supplier</h2></div>
						<div class="card-body grid grid-cols-1 md:grid-cols-2 gap-4">
							<div class="form-group"><label class="form-label required" for="pay-amt">Amount</label>
								<input id="pay-amt" class="form-control" type="number" step="0.01" min="0.01" max={toNumber(po.outstanding_amount)} bind:value={payForm.amount} required /></div>
							<div class="form-group"><label class="form-label" for="pay-m">Method</label>
								<select id="pay-m" class="form-control" bind:value={payForm.payment_method}>
									<option value="bank_transfer">Bank transfer</option><option value="upi">UPI</option>
									<option value="cash">Cash</option><option value="cheque">Cheque</option>
								</select></div>
							<div class="form-group"><label class="form-label" for="pay-ref">Reference</label>
								<input id="pay-ref" class="form-control" bind:value={payForm.transaction_reference} placeholder="UTR / cheque no." /></div>
							<div class="form-group"><label class="form-label" for="pay-notes">Notes</label>
								<input id="pay-notes" class="form-control" bind:value={payForm.notes} /></div>
							<div class="md:col-span-2 flex justify-end gap-2">
								<button type="button" class="btn btn-secondary" on:click={() => (panel = '')}>Close</button>
								<button type="submit" class="btn btn-primary" disabled={busy}>{busy ? 'Saving…' : 'Record payment'}</button>
							</div>
						</div>
					</form>
				{:else if panel === 'cancel'}
					<form class="card" on:submit|preventDefault={cancel}>
						<div class="card-header"><h2 class="font-semibold text-red-700">Cancel purchase order</h2></div>
						<div class="card-body space-y-3">
							<div class="form-group"><label class="form-label required" for="po-cancel">Reason</label>
								<input id="po-cancel" class="form-control" bind:value={cancelReason} required /></div>
							<div class="flex justify-end gap-2">
								<button type="button" class="btn btn-secondary" on:click={() => (panel = '')}>Close</button>
								<button type="submit" class="btn btn-danger" disabled={busy}>Cancel order</button>
							</div>
						</div>
					</form>
				{/if}

				<div class="card">
					<div class="card-header"><h2 class="font-semibold">Items</h2></div>
					<div class="table-container">
						<table class="table">
							<thead><tr><th>Product</th><th class="text-right">Qty</th><th class="text-right">Received</th><th class="text-right">Unit cost</th><th class="text-right">GST</th><th class="text-right">Total</th></tr></thead>
							<tbody>
								{#each po.items as it}
									<tr>
										<td><div class="font-medium">{it.product_name}</div><div class="text-xs font-mono text-slate-400">{it.sku}</div></td>
										<td class="text-right">{toNumber(it.quantity)}</td>
										<td class="text-right {toNumber(it.pending_quantity) > 0 ? 'text-amber-600' : 'text-green-600'}">{toNumber(it.received_quantity)}</td>
										<td class="text-right">{formatINR(it.unit_cost)}{toNumber(it.discount_percent) > 0 ? ` (−${toNumber(it.discount_percent)}%)` : ''}</td>
										<td class="text-right text-sm">{toNumber(it.gst_rate)}%</td>
										<td class="text-right font-semibold">{formatINR(it.total)}</td>
									</tr>
								{/each}
							</tbody>
						</table>
					</div>
				</div>

				{#if po.payments?.length}
					<div class="card">
						<div class="card-header"><h2 class="font-semibold">Payments</h2></div>
						<ul class="divide-y divide-slate-100 text-sm">
							{#each po.payments as p}
								<li class="px-5 py-3 flex justify-between">
									<span>{formatDateTime(p.created_at)} · {humanize(p.payment_method)}{p.transaction_reference ? ` · ${p.transaction_reference}` : ''}{p.notes ? ` · ${p.notes}` : ''}</span>
									<span class="font-semibold">{formatINR(p.amount)}</span>
								</li>
							{/each}
						</ul>
					</div>
				{/if}
			</div>

			<div class="space-y-6">
				<div class="card card-body space-y-2 text-sm">
					<div class="flex justify-between"><span class="text-slate-500">Taxable value</span><span>{formatINR(po.taxable_value)}</span></div>
					<div class="flex justify-between"><span class="text-slate-500">GST</span><span>{formatINR(po.gst_amount)}</span></div>
					{#if toNumber(po.freight) > 0}<div class="flex justify-between"><span class="text-slate-500">Freight</span><span>{formatINR(po.freight)}</span></div>{/if}
					{#if toNumber(po.other_charges) > 0}<div class="flex justify-between"><span class="text-slate-500">Other charges</span><span>{formatINR(po.other_charges)}</span></div>{/if}
					<div class="flex justify-between text-base font-bold border-t border-slate-200 pt-2"><span>Total</span><span>{formatINR(po.grand_total)}</span></div>
					<div class="flex justify-between text-green-700"><span>Paid</span><span>{formatINR(po.amount_paid)}</span></div>
					<div class="flex justify-between font-semibold {toNumber(po.outstanding_amount) > 0 ? 'text-red-600' : 'text-slate-400'}"><span>Outstanding</span><span>{formatINR(po.outstanding_amount)}</span></div>
				</div>
				<div class="card card-body">
					<dl class="kv">
						<div><dt>Supplier</dt><dd>{po.supplier_name}</dd></div>
						{#if po.supplier_phone}<div><dt>Phone</dt><dd>{po.supplier_phone}</dd></div>{/if}
						{#if po.supplier_gstin}<div><dt>GSTIN</dt><dd class="font-mono">{po.supplier_gstin}</dd></div>{/if}
						{#if po.supplier_invoice_number}<div><dt>Supplier invoice</dt><dd>{po.supplier_invoice_number}</dd></div>{/if}
						<div><dt>Terms</dt><dd>{po.payment_terms_days} days</dd></div>
						<div><dt>Due</dt><dd>{formatDate(po.due_date)}</dd></div>
						{#if po.created_by_name}<div><dt>Created by</dt><dd>{po.created_by_name}</dd></div>{/if}
					</dl>
					{#if po.notes}<p class="text-sm text-slate-500 whitespace-pre-line mt-4">{po.notes}</p>{/if}
				</div>
			</div>
		</div>
	{/if}
</div>
