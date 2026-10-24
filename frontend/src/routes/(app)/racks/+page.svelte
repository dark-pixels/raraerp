<script lang="ts">
	import { onMount } from 'svelte';
	import { rackApi } from '$lib/api/client';
	import { toasts } from '$stores/toasts';

	let warehouses: Record<string, unknown>[] = [];
	let racks: Record<string, unknown>[] = [];
	let loading = true;
	let activeWarehouse: string | null = null;
	let showWarehouseModal = false;
	let showRackModal = false;
	let whForm = { name: '', code: '', address: '' };
	let rackForm = { warehouse_id: '', rack_code: '', shelf: '', bin: '', zone_id: '' };
	let submitting = false;

	async function load() {
		loading = true;
		try {
			const wRes: any = await rackApi.warehouses();
			warehouses = wRes.data ?? wRes ?? [];
			if (warehouses.length > 0 && !activeWarehouse) activeWarehouse = String(warehouses[0].id);
			const rRes: any = await rackApi.racks({ warehouse_id: activeWarehouse });
			racks = rRes.data ?? rRes ?? [];
		} catch { toasts.error('Failed to load rack data'); }
		loading = false;
	}

	async function createWarehouse() {
		submitting = true;
		try {
			await rackApi.createWarehouse(whForm);
			toasts.success('Warehouse created');
			showWarehouseModal = false; whForm = { name: '', code: '', address: '' };
			await load();
		} catch (e: any) { toasts.error(e.message ?? 'Failed'); }
		submitting = false;
	}

	async function createRack() {
		submitting = true;
		try {
			const data = { ...rackForm, warehouse_id: activeWarehouse };
			await rackApi.createRack(data);
			toasts.success('Rack location created');
			showRackModal = false; rackForm = { warehouse_id: '', rack_code: '', shelf: '', bin: '', zone_id: '' };
			await load();
		} catch (e: any) { toasts.error(e.message ?? 'Failed'); }
		submitting = false;
	}

	onMount(load);
</script>

<svelte:head><title>Rack Management — RARA Enterprise ERP</title></svelte:head>

<div class="page-container">
	<div class="page-header">
		<div>
			<h1 class="page-title">Rack Management</h1>
			<p class="page-subtitle">Warehouse & storage location management</p>
		</div>
		<div class="flex gap-2">
			<button class="btn btn-secondary" on:click={() => showWarehouseModal = true}>+ Warehouse</button>
			{#if warehouses.length > 0}
				<button class="btn btn-primary" on:click={() => showRackModal = true}>+ Rack Location</button>
			{/if}
		</div>
	</div>

	{#if warehouses.length > 0}
		<div class="flex gap-2 mb-4">
			{#each warehouses as wh}
				<button class="btn {activeWarehouse === String(wh.id) ? 'btn-primary' : 'btn-secondary'}"
					on:click={() => { activeWarehouse = String(wh.id); load(); }}>
					{wh.name} ({wh.code})
				</button>
			{/each}
		</div>
	{/if}

	{#if loading}
		<div class="flex items-center justify-center py-16 text-slate-400">Loading...</div>
	{:else if warehouses.length === 0}
		<div class="card"><div class="card-body text-center py-16">
			<p class="text-slate-400 text-lg mb-2">No warehouses configured</p>
			<p class="text-slate-500 text-sm mb-4">Create a warehouse first to start managing rack locations</p>
			<button class="btn btn-primary" on:click={() => showWarehouseModal = true}>+ Add Warehouse</button>
		</div></div>
	{:else if racks.length === 0}
		<div class="card"><div class="card-body text-center py-16">
			<p class="text-slate-400 text-lg mb-2">No rack locations in this warehouse</p>
			<button class="btn btn-primary" on:click={() => showRackModal = true}>+ Add Rack Location</button>
		</div></div>
	{:else}
		<div class="grid grid-cols-3 gap-4">
		{#each racks as rack}
			<div class="card">
				<div class="card-body">
					<div class="flex items-center gap-2 mb-1">
						<span class="font-mono font-bold text-yellow-700 text-lg">{rack.rack_code}</span>
						{#if rack.shelf}<span class="text-slate-500">/ {rack.shelf}</span>{/if}
						{#if rack.bin}<span class="text-slate-500">/ {rack.bin}</span>{/if}
					</div>
					<div class="text-xs text-slate-400 mb-3">
						{rack.is_active ? '🟢 Active' : '🔴 Inactive'}
					</div>
					{#if rack.qr_code}
						<div class="text-xs text-slate-400 font-mono truncate">{rack.qr_code}</div>
					{/if}
				</div>
			</div>
		{/each}
		</div>
	{/if}
</div>

{#if showWarehouseModal}
<div class="modal-overlay" on:click|self={() => showWarehouseModal = false}>
	<div class="modal">
		<div class="modal-header"><h2 class="modal-title">Add Warehouse</h2>
			<button class="modal-close" on:click={() => showWarehouseModal = false}>✕</button></div>
		<form on:submit|preventDefault={createWarehouse}>
			<div class="modal-body">
				<div class="form-group"><label class="form-label">Name *</label><input class="input" bind:value={whForm.name} required /></div>
				<div class="form-group"><label class="form-label">Code *</label><input class="input" bind:value={whForm.code} required /></div>
				<div class="form-group"><label class="form-label">Address</label><textarea class="input" rows="2" bind:value={whForm.address}></textarea></div>
			</div>
			<div class="modal-footer">
				<button type="button" class="btn btn-secondary" on:click={() => showWarehouseModal = false}>Cancel</button>
				<button type="submit" class="btn btn-primary" disabled={submitting}>{submitting ? 'Creating...' : 'Create'}</button>
			</div>
		</form>
	</div>
</div>
{/if}

{#if showRackModal}
<div class="modal-overlay" on:click|self={() => showRackModal = false}>
	<div class="modal">
		<div class="modal-header"><h2 class="modal-title">Add Rack Location</h2>
			<button class="modal-close" on:click={() => showRackModal = false}>✕</button></div>
		<form on:submit|preventDefault={createRack}>
			<div class="modal-body">
				<div class="form-group"><label class="form-label">Rack Code *</label><input class="input" bind:value={rackForm.rack_code} required placeholder="e.g. A1" /></div>
				<div class="form-group"><label class="form-label">Shelf</label><input class="input" bind:value={rackForm.shelf} placeholder="e.g. S1" /></div>
				<div class="form-group"><label class="form-label">Bin</label><input class="input" bind:value={rackForm.bin} placeholder="e.g. B1" /></div>
			</div>
			<div class="modal-footer">
				<button type="button" class="btn btn-secondary" on:click={() => showRackModal = false}>Cancel</button>
				<button type="submit" class="btn btn-primary" disabled={submitting}>{submitting ? 'Creating...' : 'Create'}</button>
			</div>
		</form>
	</div>
</div>
{/if}
