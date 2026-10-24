<script lang="ts">
	import { onMount } from 'svelte';
	import { invoiceApi } from '$lib/api/client';
	import { toasts } from '$stores/toasts';

	let invoices: Record<string, unknown>[] = [];
	let total = 0;
	let loading = true;
	let search = '';
	let statusFilter = '';

	const formatINR = (v: unknown) =>
		new Intl.NumberFormat('en-IN', { style: 'currency', currency: 'INR', maximumFractionDigits: 2 }).format(Number(v ?? 0));
	const formatDate = (d: unknown) => d ? new Date(String(d)).toLocaleDateString('en-IN') : '—';

	const STATUS_COLORS: Record<string, string> = {
		draft: 'badge-warning', issued: 'badge-info', partially_paid: 'badge-warning',
		paid: 'badge-success', overdue: 'badge-danger', cancelled: 'badge-secondary', returned: 'badge-secondary'
	};

	async function load() {
		loading = true;
		try {
			const res: any = await invoiceApi.list({ search, status: statusFilter || undefined, limit: 50 });
			invoices = res.data ?? res ?? [];
			total = res.total ?? invoices.length;
		} catch { toasts.error('Failed to load invoices'); }
		loading = false;
	}

	onMount(load);
	$: if (search !== undefined || statusFilter !== undefined) { load(); }
</script>

<svelte:head><title>Invoices — RARA Enterprise ERP</title></svelte:head>

<div class="page-container">
	<div class="page-header">
		<div>
			<h1 class="page-title">Invoices</h1>
			<p class="page-subtitle">{total} invoice{total !== 1 ? 's' : ''} total</p>
		</div>
		<a href="/pos" class="btn btn-primary">+ New Invoice (POS)</a>
	</div>

	<div class="card mb-4">
		<div class="card-body" style="padding:1rem; display:flex; gap:1rem; flex-wrap:wrap;">
			<input class="input" placeholder="Search invoice number, customer..." bind:value={search} style="max-width:320px;" />
			<select class="input" bind:value={statusFilter} style="max-width:200px;">
				<option value="">All Status</option>
				<option value="draft">Draft</option>
				<option value="issued">Issued</option>
				<option value="partially_paid">Partially Paid</option>
				<option value="paid">Paid</option>
				<option value="overdue">Overdue</option>
				<option value="cancelled">Cancelled</option>
			</select>
		</div>
	</div>

	{#if loading}
		<div class="flex items-center justify-center py-16 text-slate-400">Loading...</div>
	{:else if invoices.length === 0}
		<div class="card"><div class="card-body text-center py-16">
			<p class="text-slate-400 text-lg mb-2">No invoices found</p>
			<p class="text-slate-500 text-sm">Create invoices from the POS / Billing section</p>
		</div></div>
	{:else}
		<div class="card">
			<div class="table-container">
				<table class="table">
					<thead><tr>
						<th>Invoice #</th><th>Date</th><th>Customer</th>
						<th>Amount</th><th>Paid</th><th>Outstanding</th><th>Status</th><th>Actions</th>
					</tr></thead>
					<tbody>
					{#each invoices as inv}
						<tr>
							<td><span class="font-mono text-sm font-semibold text-yellow-700">{inv.invoice_number}</span></td>
							<td class="text-slate-600 text-sm">{formatDate(inv.invoice_date)}</td>
							<td>{inv.customer_name ?? inv.customer_id ?? 'Walk-in'}</td>
							<td class="font-semibold">{formatINR(inv.grand_total)}</td>
							<td class="text-green-600">{formatINR(inv.amount_paid)}</td>
							<td class="font-semibold text-red-600">{formatINR(inv.outstanding_amount)}</td>
							<td><span class="badge {STATUS_COLORS[String(inv.status)] ?? 'badge-secondary'}">{String(inv.status).replace('_', ' ')}</span></td>
							<td>
								<a href={invoiceApi.pdf(String(inv.id))} target="_blank" class="btn btn-ghost btn-sm" title="Download PDF">PDF</a>
							</td>
						</tr>
					{/each}
					</tbody>
				</table>
			</div>
		</div>
	{/if}
</div>
