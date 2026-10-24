<script lang="ts">
	import { onMount } from 'svelte';
	import { inventoryApi } from '$lib/api/client';
	import { toasts } from '$stores/toasts';

	let movements: Record<string, unknown>[] = [];
	let loading = true;
	const formatDate = (d: unknown) => d ? new Date(String(d)).toLocaleString('en-IN') : '—';

	const TYPE_COLORS: Record<string, string> = {
		opening_stock: 'badge-info', purchase_receipt: 'badge-success', sales_dispatch: 'badge-warning',
		rack_transfer: 'badge-primary', warehouse_transfer: 'badge-primary',
		customer_return: 'badge-success', supplier_return: 'badge-warning',
		damaged_stock: 'badge-danger', approved_adjustment: 'badge-secondary', stock_reconciliation: 'badge-secondary'
	};

	async function load() {
		loading = true;
		try {
			const res: any = await inventoryApi.movements({ limit: 100 });
			movements = res.data ?? res ?? [];
		} catch { toasts.error('Failed to load stock movements'); }
		loading = false;
	}

	onMount(load);
</script>

<svelte:head><title>Stock Movements — RARA Enterprise ERP</title></svelte:head>

<div class="page-container">
	<div class="page-header">
		<div>
			<h1 class="page-title">Stock Movements</h1>
			<p class="page-subtitle">Full inventory movement audit trail</p>
		</div>
	</div>

	{#if loading}
		<div class="flex items-center justify-center py-16 text-slate-400">Loading...</div>
	{:else if movements.length === 0}
		<div class="card"><div class="card-body text-center py-16">
			<p class="text-slate-400 text-lg">No stock movements yet</p>
			<p class="text-slate-500 text-sm mt-1">Movements appear as stock is received, sold, or transferred</p>
		</div></div>
	{:else}
		<div class="card">
			<div class="table-container">
				<table class="table">
					<thead><tr>
						<th>Date & Time</th><th>Product</th><th>Type</th>
						<th>Quantity</th><th>Reference</th><th>From → To</th>
					</tr></thead>
					<tbody>
					{#each movements as m}
						<tr>
							<td class="text-sm text-slate-600">{formatDate(m.timestamp)}</td>
							<td class="font-medium">{m.product_name ?? m.product_id}</td>
							<td><span class="badge {TYPE_COLORS[String(m.movement_type)] ?? 'badge-secondary'}">
								{String(m.movement_type).replace(/_/g, ' ')}
							</span></td>
							<td class="font-mono font-semibold">{m.quantity}</td>
							<td class="text-sm text-slate-500">{m.reference_document ?? '—'}</td>
							<td class="text-xs text-slate-400">
								{m.source_location_id ? `Loc ${m.source_location_id}` : 'External'} → {m.destination_location_id ? `Loc ${m.destination_location_id}` : 'External'}
							</td>
						</tr>
					{/each}
					</tbody>
				</table>
			</div>
		</div>
	{/if}
</div>
