<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { inventoryApi, rackApi, errorMessage } from '$api/client';
	import { currentUser, hasPermission } from '$stores/auth';
	import { toasts } from '$stores/toasts';
	import { formatINR, formatDateTime, formatNumber, humanize, toNumber } from '$utils/format';

	type Row = Record<string, any>;

	let product: Row | null = null;
	let movements: Row[] = [];
	let categories: Row[] = [];
	let brands: Row[] = [];
	let units: Row[] = [];
	let racks: Row[] = [];
	let loading = true;
	let saving = false;
	let editing = false;
	let form: Row = {};

	let showAdjust = false;
	let adjust = { location_id: '', mode: 'set', quantity: '', reason: '', type: 'adjustment' };
	let uploading = false;

	$: id = $page.params.id ?? '';
	$: canEdit = hasPermission($currentUser, 'inventory.update');
	$: canAdjust = hasPermission($currentUser, 'inventory.adjust');
	$: margin = product && toNumber(product.retail_price) > 0
		? ((toNumber(product.retail_price) - toNumber(product.purchase_price)) / toNumber(product.retail_price)) * 100
		: 0;

	async function load() {
		loading = true;
		try {
			const [p, m]: any[] = await Promise.all([
				inventoryApi.products.get(id),
				inventoryApi.movements({ product_id: id, per_page: 25 })
			]);
			product = p;
			movements = m.data ?? [];
		} catch (e) {
			toasts.error('Failed to load product', errorMessage(e));
		}
		loading = false;
	}

	async function startEdit() {
		if (!product) return;
		form = {
			sku: product.sku, name: product.name, description: product.description ?? '', barcode: product.barcode ?? '',
			category_id: product.category_id, brand_id: product.brand_id ?? '', unit_id: product.unit_id,
			hsn_code: product.hsn_code ?? '', manufacturer: product.manufacturer ?? '',
			purchase_price: String(toNumber(product.purchase_price)), retail_price: String(toNumber(product.retail_price)),
			wholesale_price: String(toNumber(product.wholesale_price)),
			min_selling_price: product.min_selling_price == null ? '' : String(toNumber(product.min_selling_price)),
			gst_rate: String(toNumber(product.gst_rate)), reorder_threshold: String(product.reorder_threshold),
			reorder_quantity: String(product.reorder_quantity), status: product.status
		};
		editing = true;
		if (!categories.length) {
			try {
				const [c, b, u]: any[] = await Promise.all([inventoryApi.categories.list(), inventoryApi.brands.list(), inventoryApi.units.list()]);
				categories = c.data ?? []; brands = b.data ?? []; units = u.data ?? [];
			} catch (e) { toasts.error('Failed to load lists', errorMessage(e)); }
		}
	}

	async function save() {
		saving = true;
		try {
			await inventoryApi.products.update(id, form);
			toasts.success('Product updated');
			editing = false;
			await load();
		} catch (e) { toasts.error('Could not save product', errorMessage(e)); }
		saving = false;
	}

	async function openAdjust() {
		showAdjust = true;
		adjust = { location_id: product?.locations?.[0]?.location_id ?? '', mode: 'set', quantity: '', reason: '', type: 'adjustment' };
		if (!racks.length) {
			try { racks = ((await rackApi.racks()) as any).data ?? []; } catch { racks = []; }
		}
	}

	async function submitAdjust() {
		saving = true;
		try {
			const body: Row = { product_id: id, location_id: adjust.location_id || undefined, reason: adjust.reason, type: adjust.type };
			if (adjust.mode === 'set') body.new_quantity = adjust.quantity;
			else body.quantity = adjust.mode === 'remove' ? `-${adjust.quantity}` : adjust.quantity;
			await inventoryApi.adjust(body);
			toasts.success('Stock adjusted');
			showAdjust = false;
			await load();
		} catch (e) { toasts.error('Could not adjust stock', errorMessage(e)); }
		saving = false;
	}

	async function uploadImage(e: Event) {
		const file = (e.target as HTMLInputElement).files?.[0];
		if (!file) return;
		uploading = true;
		try {
			await inventoryApi.uploadImage(file, id);
			toasts.success('Image uploaded');
			await load();
		} catch (err) { toasts.error('Upload failed', errorMessage(err)); }
		uploading = false;
	}

	onMount(async () => {
		await load();
		if ($page.url.searchParams.get('edit') && canEdit) startEdit();
	});
</script>

<svelte:head><title>{product?.name ?? 'Product'} — RARA Enterprise ERP</title></svelte:head>

<div class="page-container">
	<a href="/inventory/products" class="text-sm text-slate-500 hover:text-slate-800">← Products</a>

	{#if loading}
		<div class="flex items-center justify-center py-20"><div class="spinner"></div></div>
	{:else if !product}
		<div class="empty-state py-20"><h3>Product not found</h3></div>
	{:else}
		<div class="page-header flex items-start justify-between gap-4 flex-wrap mt-2">
			<div class="flex items-center gap-4">
				{#if product.image_url}
					<img src={product.image_url} alt={product.name} class="w-16 h-16 rounded-xl object-cover border border-slate-200" />
				{/if}
				<div>
					<h1 class="page-title">{product.name}</h1>
					<p class="page-subtitle font-mono">{product.sku}{product.barcode ? ` · ${product.barcode}` : ''}</p>
					<span class="badge {product.status === 'active' ? 'badge-success' : 'badge-warning'} mt-1">{product.status}</span>
				</div>
			</div>
			{#if !editing}
				<div class="flex gap-2 flex-wrap">
					{#if canEdit}
						<label class="btn btn-secondary cursor-pointer">
							{uploading ? 'Uploading…' : 'Upload image'}
							<input type="file" accept="image/jpeg,image/png,image/webp" class="hidden" on:change={uploadImage} disabled={uploading} />
						</label>
					{/if}
					{#if canAdjust}<button class="btn btn-secondary" on:click={openAdjust}>Adjust stock</button>{/if}
					{#if canEdit}<button class="btn btn-primary" on:click={startEdit}>Edit</button>{/if}
				</div>
			{/if}
		</div>

		{#if editing}
			<form class="card" on:submit|preventDefault={save}>
				<div class="card-header"><h2 class="font-semibold">Edit product</h2></div>
				<div class="card-body grid grid-cols-1 md:grid-cols-3 gap-4">
					<div class="form-group"><label class="form-label required" for="e-sku">SKU</label><input id="e-sku" class="form-control font-mono" bind:value={form.sku} required /></div>
					<div class="form-group md:col-span-2"><label class="form-label required" for="e-name">Name</label><input id="e-name" class="form-control" bind:value={form.name} required /></div>
					<div class="form-group"><label class="form-label" for="e-barcode">Barcode</label><input id="e-barcode" class="form-control font-mono" bind:value={form.barcode} /></div>
					<div class="form-group"><label class="form-label" for="e-hsn">HSN code</label><input id="e-hsn" class="form-control font-mono" bind:value={form.hsn_code} /></div>
					<div class="form-group"><label class="form-label" for="e-mfr">Manufacturer</label><input id="e-mfr" class="form-control" bind:value={form.manufacturer} /></div>
					<div class="form-group"><label class="form-label" for="e-cat">Category</label>
						<select id="e-cat" class="form-control" bind:value={form.category_id}>{#each categories as c}<option value={c.id}>{c.name}</option>{/each}</select></div>
					<div class="form-group"><label class="form-label" for="e-brand">Brand</label>
						<select id="e-brand" class="form-control" bind:value={form.brand_id}><option value="">—</option>{#each brands as b}<option value={b.id}>{b.name}</option>{/each}</select></div>
					<div class="form-group"><label class="form-label" for="e-unit">Unit</label>
						<select id="e-unit" class="form-control" bind:value={form.unit_id}>{#each units as u}<option value={u.id}>{u.name} ({u.abbreviation})</option>{/each}</select></div>
					<div class="form-group"><label class="form-label" for="e-cost">Purchase price (₹)</label><input id="e-cost" class="form-control" type="number" step="0.01" min="0" bind:value={form.purchase_price} /></div>
					<div class="form-group"><label class="form-label" for="e-retail">Retail price (₹)</label><input id="e-retail" class="form-control" type="number" step="0.01" min="0" bind:value={form.retail_price} /></div>
					<div class="form-group"><label class="form-label" for="e-whole">Wholesale price (₹)</label><input id="e-whole" class="form-control" type="number" step="0.01" min="0" bind:value={form.wholesale_price} /></div>
					<div class="form-group"><label class="form-label" for="e-min">Minimum selling price (₹)</label><input id="e-min" class="form-control" type="number" step="0.01" min="0" bind:value={form.min_selling_price} placeholder="No minimum" /></div>
					<div class="form-group"><label class="form-label" for="e-gst">GST rate</label>
						<select id="e-gst" class="form-control" bind:value={form.gst_rate}>{#each ['0', '5', '12', '18', '28'] as r}<option value={r}>{r}%</option>{/each}</select></div>
					<div class="form-group"><label class="form-label" for="e-status">Status</label>
						<select id="e-status" class="form-control" bind:value={form.status}><option value="active">Active</option><option value="inactive">Inactive</option><option value="discontinued">Discontinued</option></select></div>
					<div class="form-group"><label class="form-label" for="e-reorder">Reorder level</label><input id="e-reorder" class="form-control" type="number" min="0" bind:value={form.reorder_threshold} /></div>
					<div class="form-group"><label class="form-label" for="e-reqty">Reorder quantity</label><input id="e-reqty" class="form-control" type="number" min="0" bind:value={form.reorder_quantity} /></div>
					<div class="form-group md:col-span-3"><label class="form-label" for="e-desc">Description</label><textarea id="e-desc" class="form-control" rows="2" bind:value={form.description}></textarea></div>
				</div>
				<div class="modal-footer">
					<button type="button" class="btn btn-secondary" on:click={() => (editing = false)}>Cancel</button>
					<button type="submit" class="btn btn-primary" disabled={saving}>{saving ? 'Saving…' : 'Save changes'}</button>
				</div>
			</form>
		{:else}
			<div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
				<div class="lg:col-span-2 space-y-6">
					<div class="card">
						<div class="card-header flex justify-between"><h2 class="font-semibold">Stock by location</h2><span class="font-semibold">{formatNumber(product.total_stock)} {product.unit_name ?? ''}</span></div>
						{#if product.locations?.length}
							<table class="table">
								<thead><tr><th>Warehouse</th><th>Rack</th><th class="text-right">On hand</th><th class="text-right">Available</th></tr></thead>
								<tbody>
									{#each product.locations as l}
										<tr><td>{l.warehouse_name}</td><td class="font-mono">{[l.rack_code, l.shelf, l.bin].filter(Boolean).join('-')}</td>
											<td class="text-right">{formatNumber(l.on_hand)}</td><td class="text-right">{formatNumber(l.available)}</td></tr>
									{/each}
								</tbody>
							</table>
						{:else}
							<div class="card-body text-sm text-slate-400">No stock recorded. Receive a purchase order or adjust stock to add some.</div>
						{/if}
					</div>

					<div class="card">
						<div class="card-header flex justify-between"><h2 class="font-semibold">Recent movements</h2><a class="text-sm text-yellow-700" href="/stock-movements">All movements →</a></div>
						{#if movements.length}
							<table class="table">
								<thead><tr><th>When</th><th>Type</th><th class="text-right">Qty</th><th>Reference</th><th>Rack</th></tr></thead>
								<tbody>
									{#each movements as m}
										<tr>
											<td class="text-sm">{formatDateTime(m.timestamp)}</td>
											<td><span class="badge badge-secondary">{humanize(m.movement_type)}</span></td>
											<td class="text-right font-mono {m.source_location_id && !m.destination_location_id ? 'text-red-600' : 'text-green-700'}">
												{m.source_location_id && !m.destination_location_id ? '−' : '+'}{formatNumber(m.quantity)}
											</td>
											<td class="text-sm">{m.reference ?? m.reason ?? '—'}</td>
											<td class="text-xs text-slate-500">{m.source_rack ?? ''}{m.source_rack && m.destination_rack ? ' → ' : ''}{m.destination_rack ?? ''}</td>
										</tr>
									{/each}
								</tbody>
							</table>
						{:else}
							<div class="card-body text-sm text-slate-400">No movements yet.</div>
						{/if}
					</div>
				</div>

				<div class="card card-body">
					<dl class="kv" style="grid-template-columns: 1fr 1fr">
						<div><dt>Category</dt><dd>{product.category_name ?? '—'}</dd></div>
						<div><dt>Brand</dt><dd>{product.brand_name ?? '—'}</dd></div>
						<div><dt>Purchase price</dt><dd>{formatINR(product.purchase_price)}</dd></div>
						<div><dt>Retail price</dt><dd>{formatINR(product.retail_price)}</dd></div>
						<div><dt>Wholesale price</dt><dd>{formatINR(product.wholesale_price)}</dd></div>
						<div><dt>Margin</dt><dd class={margin < 0 ? 'text-red-600' : ''}>{margin.toFixed(1)}%</dd></div>
						<div><dt>GST</dt><dd>{toNumber(product.gst_rate)}%</dd></div>
						<div><dt>HSN</dt><dd class="font-mono">{product.hsn_code ?? '—'}</dd></div>
						<div><dt>Reorder level</dt><dd>{product.reorder_threshold}</dd></div>
						<div><dt>Reorder qty</dt><dd>{product.reorder_quantity}</dd></div>
					</dl>
					{#if product.description}<p class="text-sm text-slate-500 mt-4">{product.description}</p>{/if}
				</div>
			</div>
		{/if}
	{/if}
</div>

{#if showAdjust}
	<div class="modal-overlay" on:click|self={() => (showAdjust = false)} on:keydown={(e) => e.key === 'Escape' && (showAdjust = false)} role="dialog" aria-modal="true" tabindex="-1" aria-label="Adjust stock">
		<form class="modal" on:submit|preventDefault={submitAdjust}>
			<div class="modal-header">
				<h2 class="modal-title">Adjust stock</h2>
				<button type="button" class="modal-close" on:click={() => (showAdjust = false)} aria-label="Close">×</button>
			</div>
			<div class="modal-body space-y-4">
				<div class="form-group"><label class="form-label" for="a-loc">Rack</label>
					<select id="a-loc" class="form-control" bind:value={adjust.location_id}>
						<option value="">Main location</option>
						{#each racks as r}<option value={r.id}>{r.label ?? r.rack_code}</option>{/each}
					</select></div>
				<div class="grid grid-cols-2 gap-4">
					<div class="form-group"><label class="form-label" for="a-mode">Action</label>
						<select id="a-mode" class="form-control" bind:value={adjust.mode}>
							<option value="set">Set counted quantity</option>
							<option value="add">Add stock</option>
							<option value="remove">Remove stock</option>
						</select></div>
					<div class="form-group"><label class="form-label required" for="a-qty">Quantity</label>
						<input id="a-qty" class="form-control" type="number" min="0" step="any" bind:value={adjust.quantity} required /></div>
				</div>
				{#if adjust.mode === 'remove'}
					<label class="flex items-center gap-2 text-sm"><input type="checkbox" checked={adjust.type === 'damaged'} on:change={(e) => (adjust.type = e.currentTarget.checked ? 'damaged' : 'adjustment')} /> Mark as damaged stock</label>
				{/if}
				<div class="form-group"><label class="form-label required" for="a-reason">Reason</label>
					<input id="a-reason" class="form-control" bind:value={adjust.reason} placeholder="Physical count, breakage, found stock…" required /></div>
			</div>
			<div class="modal-footer">
				<button type="button" class="btn btn-secondary" on:click={() => (showAdjust = false)}>Cancel</button>
				<button type="submit" class="btn btn-primary" disabled={saving}>{saving ? 'Saving…' : 'Apply adjustment'}</button>
			</div>
		</form>
	</div>
{/if}
