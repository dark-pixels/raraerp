<script lang="ts">
	import { onMount } from 'svelte';
	import { reportsApi } from '$lib/api/client';
	import { toasts } from '$stores/toasts';

	let activeTab = 'sales';
	let data: Record<string, unknown>[] = [];
	let loading = false;
	let dateFrom = '', dateTo = '';

	const formatINR = (v: unknown) =>
		new Intl.NumberFormat('en-IN', { style: 'currency', currency: 'INR', maximumFractionDigits: 0 }).format(Number(v ?? 0));
	const formatDate = (d: unknown) => d ? new Date(String(d)).toLocaleDateString('en-IN') : '—';

	async function load() {
		loading = true;
		try {
			const params = { from: dateFrom || undefined, to: dateTo || undefined, limit: 100 };
			let res: any;
			if (activeTab === 'sales') res = await reportsApi.sales(params);
			else if (activeTab === 'purchases') res = await reportsApi.purchases(params);
			else if (activeTab === 'inventory') res = await reportsApi.inventory(params);
			else if (activeTab === 'payables') res = await reportsApi.payables(params);
			else res = await reportsApi.receivables(params);
			data = res.data ?? res ?? [];
		} catch { toasts.error('Failed to load report'); }
		loading = false;
	}

	onMount(load);
</script>

<svelte:head><title>Reports — RARA Enterprise ERP</title></svelte:head>

<div class="page-container">
	<div class="page-header">
		<h1 class="page-title">Reports</h1>
		<button class="btn btn-secondary" on:click={() => window.print()}>⬇ Export</button>
	</div>

	<div class="flex gap-2 mb-4 flex-wrap">
		{#each ['sales','purchases','inventory','payables','receivables'] as t}
			<button class="btn {activeTab===t?'btn-primary':'btn-secondary'}"
				on:click={() => { activeTab = t; load(); }}>
				{t.charAt(0).toUpperCase()+t.slice(1)}
			</button>
		{/each}
	</div>

	<div class="card mb-4">
		<div class="card-body" style="padding:1rem;display:flex;gap:1rem;align-items:flex-end;flex-wrap:wrap;">
			<div class="form-group" style="margin:0;">
				<label class="form-label">From</label>
				<input class="input" type="date" bind:value={dateFrom} />
			</div>
			<div class="form-group" style="margin:0;">
				<label class="form-label">To</label>
				<input class="input" type="date" bind:value={dateTo} />
			</div>
			<button class="btn btn-primary" on:click={load}>Apply</button>
		</div>
	</div>

	{#if loading}
		<div class="flex items-center justify-center py-16 text-slate-400">Loading report...</div>
	{:else if data.length === 0}
		<div class="card"><div class="card-body text-center py-16">
			<p class="text-slate-400 text-lg">No data for selected period</p>
		</div></div>
	{:else}
		<div class="card">
			<div class="table-container">
				<table class="table">
					<thead><tr>
						{#each Object.keys(data[0]) as col}
							<th>{col.replace(/_/g,' ')}</th>
						{/each}
					</tr></thead>
					<tbody>
					{#each data as row}
						<tr>
							{#each Object.values(row) as val}
								<td class="text-sm">{typeof val === 'number' ? formatINR(val) : formatDate(val) !== val ? formatDate(val) : String(val ?? '—')}</td>
							{/each}
						</tr>
					{/each}
					</tbody>
				</table>
			</div>
		</div>
	{/if}
</div>
