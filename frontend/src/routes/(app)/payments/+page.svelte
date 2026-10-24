<script lang="ts">
	import { onMount } from 'svelte';
	import { paymentApi } from '$lib/api/client';
	import { toasts } from '$stores/toasts';

	let payments: Record<string, unknown>[] = [];
	let total = 0;
	let loading = true;
	let directionFilter = '';

	const formatINR = (v: unknown) =>
		new Intl.NumberFormat('en-IN', { style: 'currency', currency: 'INR', maximumFractionDigits: 2 }).format(Number(v ?? 0));
	const formatDate = (d: unknown) => d ? new Date(String(d)).toLocaleDateString('en-IN') : '—';

	const METHOD_ICONS: Record<string, string> = { cash: '💵', bank_transfer: '🏦', razorpay: '💳', upi: '📱', cheque: '🗒️' };
	const STATUS_COLORS: Record<string, string> = { pending: 'badge-warning', verified: 'badge-success', failed: 'badge-danger', reversed: 'badge-secondary' };

	async function load() {
		loading = true;
		try {
			const res: any = await paymentApi.list({ direction: directionFilter || undefined, limit: 100 });
			payments = res.data ?? res ?? [];
			total = res.total ?? payments.length;
		} catch { toasts.error('Failed to load payments'); }
		loading = false;
	}

	onMount(load);
	$: if (directionFilter !== undefined) { load(); }
</script>

<svelte:head><title>Payments — RARA Enterprise ERP</title></svelte:head>

<div class="page-container">
	<div class="page-header">
		<div>
			<h1 class="page-title">Payments</h1>
			<p class="page-subtitle">{total} payment records</p>
		</div>
	</div>

	<div class="card mb-4">
		<div class="card-body" style="padding:1rem;">
			<select class="input" bind:value={directionFilter} style="max-width:200px;">
				<option value="">All Payments</option>
				<option value="inbound">Inbound (Received)</option>
				<option value="outbound">Outbound (Paid)</option>
			</select>
		</div>
	</div>

	{#if loading}
		<div class="flex items-center justify-center py-16 text-slate-400">Loading...</div>
	{:else if payments.length === 0}
		<div class="card"><div class="card-body text-center py-16">
			<p class="text-slate-400 text-lg">No payment records found</p>
		</div></div>
	{:else}
		<div class="card">
			<div class="table-container">
				<table class="table">
					<thead><tr>
						<th>Date</th><th>Direction</th><th>Method</th><th>Amount</th>
						<th>Reference</th><th>Status</th>
					</tr></thead>
					<tbody>
					{#each payments as p}
						<tr>
							<td class="text-sm">{formatDate(p.payment_date)}</td>
							<td>
								<span class="badge {p.direction === 'inbound' ? 'badge-success' : 'badge-warning'}">
									{p.direction === 'inbound' ? '↓ In' : '↑ Out'}
								</span>
							</td>
							<td>{METHOD_ICONS[String(p.payment_method)] ?? ''} {p.payment_method}</td>
							<td class="font-semibold text-lg">{formatINR(p.amount)}</td>
							<td class="font-mono text-sm text-slate-500">{p.transaction_reference ?? '—'}</td>
							<td><span class="badge {STATUS_COLORS[String(p.status)] ?? 'badge-secondary'}">{p.status}</span></td>
						</tr>
					{/each}
					</tbody>
				</table>
			</div>
		</div>
	{/if}
</div>
