<script lang="ts">
	import { onMount } from 'svelte';
	import { rackApi, inventoryApi, errorMessage } from '$api/client';
	import { currentUser, hasPermission } from '$stores/auth';
	import { toasts } from '$stores/toasts';
	import { formatNumber, debounce, toNumber } from '$utils/format';

	type Row = Record<string, any>;

	let warehouses: Row[] = [];
	let racks: Row[] = [];
	let loading = true;
	let activeWarehouse: string | null = null;
	let showWarehouseModal = false;
	let showRackModal = false;
	let whForm = { name: '', code: '', address: '' };
	let rackForm = { rack_code: '', shelf: '', bin: '' };
	let submitting = false;

	// Rack contents panel
	let openRack: Row | null = null;
	let items: Row[] = [];
	let itemsLoading = false;
	let counts: Record<string, string> = {};
	let reason = 'Daily rack count';
	let saving = false;

	// Add product to rack
	let addSearch = '';
	let addResults: Row[] = [];
	let addProduct: Row | null = null;
	let addQty = '';

	// Move to another rack
	let moving: Row | null = null;
	let moveTo = '';
	let moveQty = '';

	$: canAdjust = hasPermission($currentUser, 'inventory.adjust');
	$: canTransfer = hasPermission($currentUser, 'inventory.transfer');
	$: canAssign = hasPermission($currentUser, 'inventory.rack.assign');
	$: canManageWarehouses = hasPermission($currentUser, 'settings.manage');
	$: changed = items.filter((i) => counts[i.product_id] !== undefined && counts[i.product_id] !== '' && toNumber(counts[i.product_id]) !== toNumber(i.on_hand));
	$: otherRacks = racks.filter((r) => r.id !== openRack?.id);

	const rackLabel = (r: Row | null) => (r ? [r.rack_code, r.shelf, r.bin].filter(Boolean).join(' / ') : '');

	async function load() {
		loading = true;
		try {
			const wRes: any = await rackApi.warehouses();
			warehouses = wRes.data ?? [];
			if (warehouses.length > 0 && !activeWarehouse) activeWarehouse = String(warehouses[0].id);
			const rRes: any = await rackApi.racks({ warehouse_id: activeWarehouse });
			racks = rRes.data ?? [];
		} catch (e) { toasts.error('Failed to load racks', errorMessage(e)); }
		loading = false;
	}

	async function createWarehouse() {
		submitting = true;
		try {
			await rackApi.createWarehouse(whForm);
			toasts.success('Warehouse created');
			showWarehouseModal = false; whForm = { name: '', code: '', address: '' };
			await load();
		} catch (e) { toasts.error('Could not create warehouse', errorMessage(e)); }
		submitting = false;
	}

	async function createRack() {
		submitting = true;
		try {
			await rackApi.createRack({ ...rackForm, warehouse_id: activeWarehouse });
			toasts.success('Rack location created');
			showRackModal = false; rackForm = { rack_code: '', shelf: '', bin: '' };
			await load();
		} catch (e) { toasts.error('Could not create rack', errorMessage(e)); }
		submitting = false;
	}

	async function loadItems() {
		if (!openRack) return;
		itemsLoading = true;
		try {
			const res: any = await rackApi.rackInventory(openRack.id);
			items = res.data ?? [];
			counts = Object.fromEntries(items.map((i) => [i.product_id, String(toNumber(i.on_hand))]));
		} catch (e) { toasts.error('Failed to load rack items', errorMessage(e)); }
		itemsLoading = false;
	}

	function openPanel(r: Row) {
		openRack = r;
		addProduct = null; addSearch = ''; addResults = []; addQty = ''; moving = null;
		loadItems();
	}

	function closePanel() { openRack = null; load(); }

	async function setQuantity(productId: string, qty: string, why: string) {
		await inventoryApi.adjust({ product_id: productId, location_id: openRack!.id, new_quantity: qty, reason: why });
	}

	async function saveCounts() {
		if (!changed.length) return;
		if (!reason.trim()) { toasts.warning('Enter a reason for the change'); return; }
		saving = true;
		let ok = 0;
		for (const it of changed) {
			try {
				await setQuantity(it.product_id, counts[it.product_id], reason.trim());
				ok++;
			} catch (e) { toasts.error(`${it.product_name}: not saved`, errorMessage(e)); }
		}
		if (ok) toasts.success(`Updated ${ok} item${ok === 1 ? '' : 's'}`);
		saving = false;
		await loadItems();
	}

	async function removeItem(it: Row) {
		if (!confirm(`Remove all ${formatNumber(it.on_hand)} of ${it.product_name} from this rack? Stock will be written off.\nTo move it instead, use Move.`)) return;
		try {
			await setQuantity(it.product_id, '0', 'Removed from rack');
			toasts.success(`${it.product_name} removed from rack`);
			await loadItems();
		} catch (e) { toasts.error('Could not remove', errorMessage(e)); }
	}

	const searchProducts = debounce(async () => {
		if (addSearch.trim().length < 2) { addResults = []; return; }
		try {
			const res: any = await inventoryApi.products.list({ search: addSearch.trim(), status: 'active', per_page: 8 });
			addResults = res.data ?? [];
		} catch { addResults = []; }
	}, 250);

	async function addToRack() {
		if (!addProduct || toNumber(addQty) <= 0) { toasts.warning('Choose a product and quantity'); return; }
		const existing = items.find((i) => i.product_id === addProduct!.id);
		const target = toNumber(existing?.on_hand) + toNumber(addQty);
		saving = true;
		try {
			await inventoryApi.adjust({
				product_id: addProduct.id, location_id: openRack!.id, quantity: addQty,
				reason: 'Stock placed in rack', type: 'received'
			});
			toasts.success(`${addProduct.name}: ${formatNumber(target)} in rack now`);
			addProduct = null; addSearch = ''; addQty = ''; addResults = [];
			await loadItems();
		} catch (e) { toasts.error('Could not add to rack', errorMessage(e)); }
		saving = false;
	}

	async function moveItem() {
		if (!moving || !moveTo || toNumber(moveQty) <= 0) { toasts.warning('Choose a rack and quantity'); return; }
		saving = true;
		try {
			await rackApi.transfer({
				product_id: moving.product_id, from_location_id: openRack!.id, to_location_id: moveTo,
				quantity: moveQty, reason: 'Moved between racks'
			});
			toasts.success(`Moved ${moveQty} × ${moving.product_name}`);
			moving = null;
			await loadItems();
		} catch (e) { toasts.error('Could not move stock', errorMessage(e)); }
		saving = false;
	}

	function onKey(e: KeyboardEvent) { if (e.key === 'Escape' && openRack) closePanel(); }

	onMount(load);
</script>

<svelte:window on:keydown={onKey} />
<svelte:head><title>Rack Management — RARA Enterprise ERP</title></svelte:head>

<div class="page-container">
	<div class="page-header flex items-start justify-between gap-4 flex-wrap">
		<div>
			<h1 class="page-title">Rack Management</h1>
			<p class="page-subtitle">Click a rack to see and update what is stored in it. Sales reduce rack stock automatically.</p>
		</div>
		<div class="flex gap-2">
			{#if canManageWarehouses}<button class="btn btn-secondary" on:click={() => (showWarehouseModal = true)}>+ Warehouse</button>{/if}
			{#if warehouses.length > 0 && canAssign}
				<button class="btn btn-primary" on:click={() => (showRackModal = true)}>+ Rack Location</button>
			{/if}
		</div>
	</div>

	{#if warehouses.length > 1}
		<div class="flex gap-2 mb-4 flex-wrap">
			{#each warehouses as wh}
				<button class="btn {activeWarehouse === String(wh.id) ? 'btn-primary' : 'btn-secondary'}"
					on:click={() => { activeWarehouse = String(wh.id); load(); }}>
					{wh.name} ({wh.code})
				</button>
			{/each}
		</div>
	{/if}

	{#if loading}
		<div class="flex items-center justify-center py-16"><div class="spinner"></div></div>
	{:else if warehouses.length === 0}
		<div class="card"><div class="card-body text-center py-16">
			<p class="text-slate-400 text-lg mb-2">No warehouses configured</p>
			<p class="text-slate-500 text-sm mb-4">Create a warehouse first to start managing rack locations</p>
			{#if canManageWarehouses}<button class="btn btn-primary" on:click={() => (showWarehouseModal = true)}>+ Add Warehouse</button>{/if}
		</div></div>
	{:else if racks.length === 0}
		<div class="card"><div class="card-body text-center py-16">
			<p class="text-slate-400 text-lg mb-2">No rack locations in this warehouse</p>
			{#if canAssign}<button class="btn btn-primary" on:click={() => (showRackModal = true)}>+ Add Rack Location</button>{/if}
		</div></div>
	{:else}
		<div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
			{#each racks as rack (rack.id)}
				<button type="button" class="card text-left hover:shadow-md hover:border-yellow-300 transition-all" on:click={() => openPanel(rack)}>
					<div class="card-body">
						<div class="flex items-start justify-between gap-2">
							<div class="font-mono font-bold text-yellow-700 text-lg">{rackLabel(rack)}</div>
							<span class="badge {rack.is_active ? 'badge-success' : 'badge-secondary'}">{rack.is_active ? 'Active' : 'Inactive'}</span>
						</div>
						<div class="flex gap-6 mt-3 text-sm">
							<div><div class="text-xs uppercase tracking-wide text-slate-400 font-semibold">Products</div><div class="font-semibold text-slate-800">{rack.product_count ?? 0}</div></div>
							<div><div class="text-xs uppercase tracking-wide text-slate-400 font-semibold">Units</div><div class="font-semibold text-slate-800">{formatNumber(rack.total_units)}</div></div>
						</div>
						<div class="text-xs text-yellow-700 font-medium mt-3">View &amp; edit items →</div>
					</div>
				</button>
			{/each}
		</div>
	{/if}
</div>

{#if openRack}
	<div class="drawer-overlay" on:click={closePanel} role="presentation"></div>
	<div class="drawer" role="dialog" aria-modal="true" aria-label="Rack contents" tabindex="-1">
		<div class="drawer-header">
			<div>
				<p class="text-xs uppercase tracking-wide text-slate-400 font-semibold">{openRack.warehouse_name}</p>
				<h2 class="text-xl font-bold font-mono text-yellow-700">{rackLabel(openRack)}</h2>
				<p class="text-sm text-slate-500">{items.length} product{items.length === 1 ? '' : 's'} · {formatNumber(items.reduce((s, i) => s + toNumber(i.on_hand), 0))} units</p>
			</div>
			<button class="modal-close" on:click={closePanel} aria-label="Close">×</button>
		</div>

		<div class="drawer-body space-y-5">
			{#if canAdjust}
				<div class="rounded-xl border border-slate-200 p-4 space-y-3">
					<h3 class="font-semibold text-slate-800">Add product to this rack</h3>
					{#if addProduct}
						<div class="flex items-center justify-between bg-yellow-50 rounded-lg px-3 py-2">
							<div><div class="font-medium">{addProduct.name}</div><div class="text-xs text-slate-500 font-mono">{addProduct.sku} · total stock {formatNumber(addProduct.total_stock)}</div></div>
							<button class="btn btn-ghost btn-sm" on:click={() => (addProduct = null)} aria-label="Change product">×</button>
						</div>
						<div class="flex gap-2 items-end">
							<div class="form-group flex-1"><label class="form-label" for="add-qty">Quantity to add</label>
								<input id="add-qty" class="form-control" type="number" min="0" step="any" bind:value={addQty} placeholder="e.g. 50" /></div>
							<button class="btn btn-primary" disabled={saving} on:click={addToRack}>Add</button>
						</div>
					{:else}
						<div class="relative">
							<input class="form-control" placeholder="Search product by name, SKU or barcode…" bind:value={addSearch} on:input={searchProducts} aria-label="Search product to add" />
							{#if addResults.length}
								<div class="absolute z-10 left-0 right-0 mt-1 bg-white border border-slate-200 rounded-xl shadow-lg max-h-64 overflow-auto">
									{#each addResults as p}
										<button type="button" class="w-full text-left px-3 py-2 hover:bg-yellow-50 border-b border-slate-50 last:border-0"
											on:click={() => { addProduct = p; addResults = []; }}>
											<div class="text-sm font-medium">{p.name}</div>
											<div class="text-xs text-slate-500 font-mono">{p.sku} · stock {formatNumber(p.total_stock)}</div>
										</button>
									{/each}
								</div>
							{/if}
						</div>
					{/if}
				</div>
			{/if}

			<div>
				<h3 class="font-semibold text-slate-800 mb-2">Items in this rack</h3>
				{#if itemsLoading}
					<div class="flex justify-center py-8"><div class="spinner"></div></div>
				{:else if items.length === 0}
					<p class="text-sm text-slate-400 py-6 text-center">This rack is empty. Add products above.</p>
				{:else}
					<div class="table-container border border-slate-100 rounded-xl">
						<table class="table">
							<thead><tr><th>Product</th><th class="text-right">System qty</th><th style="width:120px">Counted qty</th><th></th></tr></thead>
							<tbody>
								{#each items as it (it.product_id)}
									<tr class={counts[it.product_id] !== undefined && toNumber(counts[it.product_id]) !== toNumber(it.on_hand) ? 'bg-yellow-50' : ''}>
										<td><div class="font-medium text-sm">{it.product_name}</div><div class="text-xs text-slate-400 font-mono">{it.sku}</div></td>
										<td class="text-right font-mono">{formatNumber(it.on_hand)}</td>
										<td>
											<input class="form-control" type="number" min="0" step="any" bind:value={counts[it.product_id]}
												disabled={!canAdjust} aria-label="Counted quantity for {it.product_name}" />
										</td>
										<td class="text-right whitespace-nowrap">
											{#if canTransfer && otherRacks.length}
												<button class="btn btn-ghost btn-sm" on:click={() => { moving = it; moveTo = ''; moveQty = String(toNumber(it.on_hand)); }}>Move</button>
											{/if}
											{#if canAdjust}
												<button class="btn btn-ghost btn-sm text-red-500" on:click={() => removeItem(it)}>Remove</button>
											{/if}
										</td>
									</tr>
									{#if moving?.product_id === it.product_id}
										<tr>
											<td colspan="4" class="bg-slate-50">
												<div class="flex gap-2 items-end flex-wrap">
													<div class="form-group"><label class="form-label" for="mv-to">Move to rack</label>
														<select id="mv-to" class="form-control" bind:value={moveTo}>
															<option value="">Select rack</option>
															{#each otherRacks as r}<option value={r.id}>{rackLabel(r)}</option>{/each}
														</select></div>
													<div class="form-group"><label class="form-label" for="mv-qty">Quantity</label>
														<input id="mv-qty" class="form-control" type="number" min="0" step="any" max={toNumber(it.on_hand)} bind:value={moveQty} style="width:100px" /></div>
													<button class="btn btn-primary btn-sm" disabled={saving} on:click={moveItem}>Move</button>
													<button class="btn btn-ghost btn-sm" on:click={() => (moving = null)}>Cancel</button>
												</div>
											</td>
										</tr>
									{/if}
								{/each}
							</tbody>
						</table>
					</div>
				{/if}
			</div>
		</div>

		{#if canAdjust && items.length}
			<div class="drawer-footer items-end">
				<div class="form-group flex-1" style="min-width:200px">
					<label class="form-label" for="count-reason">Reason for changes</label>
					<input id="count-reason" class="form-control" bind:value={reason} />
				</div>
				<button class="btn btn-secondary" disabled={!changed.length || saving} on:click={loadItems}>Reset</button>
				<button class="btn btn-primary" disabled={!changed.length || saving} on:click={saveCounts}>
					{saving ? 'Saving…' : changed.length ? `Save ${changed.length} change${changed.length === 1 ? '' : 's'}` : 'No changes'}
				</button>
			</div>
		{/if}
	</div>
{/if}

{#if showWarehouseModal}
	<div class="modal-overlay" role="presentation" on:click|self={() => (showWarehouseModal = false)}>
		<form class="modal" on:submit|preventDefault={createWarehouse}>
			<div class="modal-header"><h2 class="modal-title">Add Warehouse</h2>
				<button type="button" class="modal-close" on:click={() => (showWarehouseModal = false)} aria-label="Close">×</button></div>
			<div class="modal-body space-y-4">
				<div class="form-group"><label class="form-label required" for="wh-name">Name</label><input id="wh-name" class="input" bind:value={whForm.name} required /></div>
				<div class="form-group"><label class="form-label required" for="wh-code">Code</label><input id="wh-code" class="input uppercase" bind:value={whForm.code} required /></div>
				<div class="form-group"><label class="form-label" for="wh-addr">Address</label><textarea id="wh-addr" class="input" rows="2" bind:value={whForm.address}></textarea></div>
			</div>
			<div class="modal-footer">
				<button type="button" class="btn btn-secondary" on:click={() => (showWarehouseModal = false)}>Cancel</button>
				<button type="submit" class="btn btn-primary" disabled={submitting}>{submitting ? 'Creating…' : 'Create'}</button>
			</div>
		</form>
	</div>
{/if}

{#if showRackModal}
	<div class="modal-overlay" role="presentation" on:click|self={() => (showRackModal = false)}>
		<form class="modal" style="max-width:420px" on:submit|preventDefault={createRack}>
			<div class="modal-header"><h2 class="modal-title">Add Rack Location</h2>
				<button type="button" class="modal-close" on:click={() => (showRackModal = false)} aria-label="Close">×</button></div>
			<div class="modal-body space-y-4">
				<div class="form-group"><label class="form-label required" for="rk-code">Rack code</label><input id="rk-code" class="input uppercase" bind:value={rackForm.rack_code} required placeholder="e.g. A1" /></div>
				<div class="grid grid-cols-2 gap-3">
					<div class="form-group"><label class="form-label" for="rk-shelf">Shelf</label><input id="rk-shelf" class="input" bind:value={rackForm.shelf} placeholder="e.g. S1" /></div>
					<div class="form-group"><label class="form-label" for="rk-bin">Bin</label><input id="rk-bin" class="input" bind:value={rackForm.bin} placeholder="e.g. B1" /></div>
				</div>
			</div>
			<div class="modal-footer">
				<button type="button" class="btn btn-secondary" on:click={() => (showRackModal = false)}>Cancel</button>
				<button type="submit" class="btn btn-primary" disabled={submitting}>{submitting ? 'Creating…' : 'Create'}</button>
			</div>
		</form>
	</div>
{/if}
