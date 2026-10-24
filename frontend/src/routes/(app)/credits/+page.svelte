<script lang="ts">
	import { onMount } from 'svelte';
	import { customerApi } from '$lib/api/client';
	import { toasts } from '$stores/toasts';

	let customers: Record<string, unknown>[] = [];
	let loading = true;

	const formatINR = (v: unknown) =>
		new Intl.NumberFormat('en-IN', { style: 'currency', currency: 'INR', maximumFractionDigits: 0 }).format(Number(v ?? 0));

	async function load() {
		loading = true;
		try {
			const res: any = await customerApi.list({ limit: 100 });
			const all = res.data ?? res ?? [];
			customers = all.filter((c: Record<string, unknown>) => Number(c.outstanding_balance) > 0 || Number(c.credit_limit) > 0);
		} catch { toasts.error('Failed to load credit data'); }
		loading = false;
	}

	onMount(load);
</script>

<svelte:head><title>Credit Management — RARA Enterprise ERP</title></svelte:head>

<div class="page-container">
	<div class="page-header">
		<div>
			<h1 class="page-title">Credit Management</h1>
			<p class="page-subtitle">Customer outstanding balances and credit limits</p>
		</div>
	</div>

	{#if loading}
		<div class="flex items-center justify-center py-16 text-slate-400">Loading...</div>
	{:else if customers.length === 0}
		<div class="card"><div class="card-body text-center py-16">
			<p class="text-slate-400 text-lg">No credit accounts</p>
			<p class="text-slate-500 text-sm mt-1">Customers with active credit will appear here</p>
		</div></div>
	{:else}
		<div class="grid grid-cols-3 gap-4 mb-6">
			<div class="card"><div class="card-body">
				<p class="text-xs font-semibold uppercase tracking-widest text-red-500 mb-1">Total Outstanding</p>
				<p class="text-2xl font-bold text-red-600">{formatINR(customers.reduce((s, c) => s + Number(c.outstanding_balance ?? 0), 0))}</p>
			</div></div>
			<div class="card"><div class="card-body">
				<p class="text-xs font-semibold uppercase tracking-widest text-slate-500 mb-1">Total Credit Limit</p>
				<p class="text-2xl font-bold text-slate-700">{formatINR(customers.reduce((s, c) => s + Number(c.credit_limit ?? 0), 0))}</p>
			</div></div>
			<div class="card"><div class="card-body">
				<p class="text-xs font-semibold uppercase tracking-widest text-yellow-500 mb-1">Credit Accounts</p>
				<p class="text-2xl font-bold text-yellow-700">{customers.length}</p>
			</div></div>
		</div>
		<div class="card">
			<div class="table-container">
				<table class="table">
					<thead><tr>
						<th>Customer</th><th>Type</th><th>Credit Limit</th>
						<th>Outstanding</th><th>Used %</th><th>Period</th>
					</tr></thead>
					<tbody>
					{#each customers as c}
						{@const used = Number(c.credit_limit) > 0 ? (Number(c.outstanding_balance) / Number(c.credit_limit)) * 100 : 0}
						<tr>
							<td>
								<div class="font-semibold">{c.name}</div>
								{#if c.business_name}<div class="text-xs text-slate-500">{c.business_name}</div>{/if}
							</td>
							<td><span class="badge badge-{c.customer_type === 'wholesale' ? 'primary' : 'info'}">{c.customer_type}</span></td>
							<td class="font-medium">{formatINR(c.credit_limit)}</td>
							<td class="font-semibold text-red-600">{formatINR(c.outstanding_balance)}</td>
							<td>
								<div class="flex items-center gap-2">
									<div style="width:80px;height:6px;background:#e2e8f0;border-radius:3px;overflow:hidden;">
										<div style="width:{Math.min(used,100)}%;height:100%;background:{used>80?'#ef4444':used>50?'#f59e0b':'#22c55e'};border-radius:3px;"></div>
									</div>
									<span class="text-xs {used > 80 ? 'text-red-600 font-bold' : 'text-slate-500'}">{used.toFixed(0)}%</span>
								</div>
							</td>
							<td class="text-sm text-slate-500">{c.credit_period_days} days</td>
						</tr>
					{/each}
					</tbody>
				</table>
			</div>
		</div>
	{/if}
</div>
