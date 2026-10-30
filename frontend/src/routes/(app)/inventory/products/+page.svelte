<script lang="ts">
	import { onMount } from 'svelte';
	import { inventoryApi, rackApi, errorMessage } from '$api/client';
	import { toasts } from '$stores/toasts';
	import { currentUser, hasPermission } from '$stores/auth';

	// Receive stock for an existing product
	let stockFor: Record<string, unknown> | null = null;
	let stockRacks: Record<string, unknown>[] = [];
	let stockForm = { quantity: '', location_id: '', reason: 'Stock received' };
	let savingStock = false;
	$: canAdjustStock = hasPermission($currentUser, 'inventory.adjust');

	async function openAddStock(p: Record<string, unknown>) {
		stockFor = p;
		stockForm = { quantity: '', location_id: '', reason: 'Stock received' };
		if (!stockRacks.length) {
			try { stockRacks = ((await rackApi.racks()) as { data: Record<string, unknown>[] }).data ?? []; } catch { stockRacks = []; }
		}
	}

	async function saveAddStock() {
		if (!stockFor) return;
		if (!(Number(stockForm.quantity) > 0)) { toasts.warning('Enter a quantity greater than zero'); return; }
		savingStock = true;
		try {
			await inventoryApi.adjust({
				product_id: stockFor.id,
				location_id: stockForm.location_id || undefined,
				quantity: stockForm.quantity,
				reason: stockForm.reason || 'Stock received',
				type: 'received'
			});
			toasts.success(`Added ${stockForm.quantity} to ${stockFor.name}`);
			stockFor = null;
			await loadProducts();
		} catch (e) {
			toasts.error('Could not add stock', errorMessage(e));
		}
		savingStock = false;
	}

	let products: Record<string, unknown>[] = [];
	let categories: Record<string, unknown>[] = [];
	let brands: Record<string, unknown>[] = [];
	let units: Record<string, unknown>[] = [];
	let total = 0;
	let page = 1;
	let perPage = 20;
	let loading = true;
	let search = '';
	let selectedCategory = '';
	let selectedStatus = '';
	let showAddModal = false;
	let submitting = false;
	let searchTimeout: ReturnType<typeof setTimeout>;

	// Quick-create state
	let showCatModal = false, newCatName = '', savingCat = false;
	let showBrandModal = false, newBrandName = '', savingBrand = false;
	let showUnitModal = false, newUnitName = '', newUnitAbbr = '', savingUnit = false;

	const newProduct = {
		sku: '', name: '', description: '',
		category_id: '', brand_id: '',
		unit_id: '', hsn_code: '',
		purchase_price: '', retail_price: '', wholesale_price: '',
		gst_rate: '18', reorder_threshold: '10', reorder_quantity: '50',
		barcode: '', opening_stock: ''
	};
	const isBlank = (v: unknown) => v === '' || v === null || v === undefined;

	const formatINR = (v: unknown) => new Intl.NumberFormat('en-IN', { style: 'currency', currency: 'INR', maximumFractionDigits: 2 }).format(Number(v ?? 0));

	// Load dropdowns independently so products 500 doesn't break them
	async function loadDropdowns() {
		try {
			const [cats, brnds, unts] = await Promise.all([
				inventoryApi.categories.list(),
				inventoryApi.brands.list(),
				inventoryApi.units.list()
			]);
			categories = (cats as { data: Record<string, unknown>[] }).data ?? (cats as Record<string, unknown>[]) ?? [];
			brands = (brnds as { data: Record<string, unknown>[] }).data ?? (brnds as Record<string, unknown>[]) ?? [];
			units = (unts as { data: Record<string, unknown>[] }).data ?? (unts as Record<string, unknown>[]) ?? [];
		} catch { /* silent - dropdowns will be empty but page still loads */ }
	}

	async function loadProducts() {
		loading = true;
		try {
			const prods = await inventoryApi.products.list({ page, per_page: perPage, search: search || undefined, category_id: selectedCategory || undefined, status: selectedStatus || undefined });
			const pr = prods as { data: Record<string, unknown>[]; total: number };
			products = pr.data ?? [];
			total = pr.total ?? 0;
		} catch (e: unknown) {
			const err = e as { message?: string };
			toasts.error('Failed to load products: ' + (err.message ?? 'Server error'));
			products = [];
		} finally {
			loading = false;
		}
	}

	async function loadData() { await loadProducts(); }

	function onSearchInput() {
		clearTimeout(searchTimeout);
		searchTimeout = setTimeout(() => { page = 1; loadProducts(); }, 400);
	}

	// Quick-create handlers
	async function createCategory() {
		if (!newCatName.trim()) return;
		savingCat = true;
		try {
			const res = await inventoryApi.categories.create({ name: newCatName.trim() }) as { id: string };
			toasts.success(`Category "${newCatName}" created`);
			newCatName = '';
			showCatModal = false;
			await loadDropdowns();
			newProduct.category_id = res.id; // auto-select new category
		} catch (e: any) { toasts.error(e.message ?? 'Failed to create category'); }
		savingCat = false;
	}

	async function createBrand() {
		if (!newBrandName.trim()) return;
		savingBrand = true;
		try {
			const res = await inventoryApi.brands.create({ name: newBrandName.trim() }) as { id: string };
			toasts.success(`Brand "${newBrandName}" created`);
			newBrandName = '';
			showBrandModal = false;
			await loadDropdowns();
			newProduct.brand_id = res.id;
		} catch (e: any) { toasts.error(e.message ?? 'Failed to create brand'); }
		savingBrand = false;
	}

	async function createUnit() {
		if (!newUnitName.trim() || !newUnitAbbr.trim()) { toasts.error('Name and abbreviation required'); return; }
		savingUnit = true;
		try {
			const res = await inventoryApi.units.create({
				name: newUnitName.trim(),
				abbreviation: newUnitAbbr.trim()
			}) as { id: string };
			toasts.success(`Unit "${newUnitName}" created`);
			newUnitName = ''; newUnitAbbr = '';
			showUnitModal = false;
			await loadDropdowns();
			newProduct.unit_id = res.id;
		} catch (e: any) { toasts.error(e.message ?? 'Failed to create unit'); }
		savingUnit = false;
	}

	async function createProduct() {
		if (!newProduct.sku.trim()) { toasts.error('SKU is required'); return; }
		if (!newProduct.name.trim()) { toasts.error('Product name is required'); return; }
		if (!newProduct.category_id) { toasts.error('Category is required'); return; }
		if (!newProduct.unit_id) { toasts.error('Unit of measure is required'); return; }
		if (isBlank(newProduct.purchase_price)) { toasts.error('Purchase price is required'); return; }
		if (isBlank(newProduct.retail_price)) { toasts.error('Retail price is required'); return; }
		if (isBlank(newProduct.wholesale_price)) { toasts.error('Wholesale price is required'); return; }

		submitting = true;
		try {
			const payload = {
				sku: newProduct.sku.trim(), name: newProduct.name.trim(),
				description: newProduct.description.trim() || null,
				category_id: newProduct.category_id, brand_id: newProduct.brand_id || null,
				unit_id: newProduct.unit_id, hsn_code: newProduct.hsn_code.trim() || null,
				purchase_price: Number(newProduct.purchase_price),
				retail_price: Number(newProduct.retail_price),
				wholesale_price: Number(newProduct.wholesale_price),
				gst_rate: Number(newProduct.gst_rate),
				reorder_threshold: Number(newProduct.reorder_threshold) || 10,
				reorder_quantity: Number(newProduct.reorder_quantity) || 50,
				barcode: String(newProduct.barcode ?? '').trim() || null,
				opening_stock: isBlank(newProduct.opening_stock) ? undefined : Number(newProduct.opening_stock),
			};
			await inventoryApi.products.create(payload);
			toasts.success('Product created successfully');
			showAddModal = false;
			newProduct.sku = ''; newProduct.name = ''; newProduct.description = '';
			newProduct.category_id = ''; newProduct.brand_id = ''; newProduct.unit_id = '';
			newProduct.hsn_code = ''; newProduct.purchase_price = ''; newProduct.retail_price = '';
			newProduct.wholesale_price = ''; newProduct.gst_rate = '18';
			newProduct.barcode = ''; newProduct.opening_stock = '';
			loadProducts();
		} catch (e: unknown) {
			const err = e as { message?: string };
			toasts.error('Failed to create product: ' + (err.message ?? 'Unknown error'));
		} finally {
			submitting = false;
		}
	}

	onMount(() => {
		loadDropdowns();
		loadProducts();
	});

	$: totalPages = Math.ceil(total / perPage);

	const GST_RATES = ['0', '5', '12', '18', '28'];
</script>

<svelte:head>
	<title>Products — RARA Enterprise ERP</title>
</svelte:head>

<div class="page-container">
	<div class="page-header">
		<div>
			<h1 class="page-title">Products & Inventory</h1>
			<p class="page-subtitle">{total.toLocaleString('en-IN')} products in catalogue</p>
		</div>
		<div class="flex gap-2">
			<a href="/inventory/import" class="btn btn-secondary">
				<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
					<polyline points="16 16 12 12 8 16"/><line x1="12" y1="12" x2="12" y2="21"/>
					<path d="M20.39 18.39A5 5 0 0018 9h-1.26A8 8 0 103 16.3"/>
				</svg>
				Bulk Import
			</a>
			<button class="btn btn-primary" on:click={() => showAddModal = true}>
				<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
					<line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>
				</svg>
				Add Product
			</button>
		</div>
	</div>

	<!-- Filters -->
	<div class="card mb-5">
		<div class="card-body">
			<div class="grid grid-cols-1 md:grid-cols-4 gap-3">
				<div class="form-group mb-0 md:col-span-2">
					<div class="relative">
						<svg class="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/>
						</svg>
						<input
							type="text"
							class="form-control pl-9"
							placeholder="Search by name or SKU…"
							bind:value={search}
							on:input={onSearchInput}
						/>
					</div>
				</div>
				<div class="form-group mb-0">
					<select class="form-control" bind:value={selectedCategory} on:change={() => { page = 1; loadData(); }}>
						<option value="">All Categories</option>
						{#each categories as c}
							<option value={c.id}>{c.name}</option>
						{/each}
					</select>
				</div>
				<div class="form-group mb-0">
					<select class="form-control" bind:value={selectedStatus} on:change={() => { page = 1; loadData(); }}>
						<option value="">All Statuses</option>
						<option value="active">Active</option>
						<option value="inactive">Inactive</option>
						<option value="discontinued">Discontinued</option>
					</select>
				</div>
			</div>
		</div>
	</div>

	<!-- Products table -->
	<div class="card">
		{#if loading}
			<div class="p-8 flex items-center justify-center">
				<div class="spinner mr-3"></div>
				<span class="text-slate-500">Loading products…</span>
			</div>
		{:else if products.length === 0}
			<div class="empty-state py-16">
				<div class="empty-state-icon">
					<svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
						<path d="M21 16V8a2 2 0 00-1-1.73l-7-4a2 2 0 00-2 0l-7 4A2 2 0 003 8v8a2 2 0 001 1.73l7 4a2 2 0 002 0l7-4A2 2 0 0021 16z"/>
					</svg>
				</div>
				<h3>No products found</h3>
				<p>Add your first product or use bulk import to get started.</p>
				<button class="btn btn-primary mt-4" on:click={() => showAddModal = true}>Add Product</button>
			</div>
		{:else}
			<div class="table-container">
				<table class="data-table">
					<thead>
						<tr>
							<th>Product</th>
							<th>SKU</th>
							<th>Category</th>
							<th class="text-right">Purchase Price</th>
							<th class="text-right">Retail Price</th>
							<th class="text-right">Stock</th>
							<th>GST</th>
							<th>Status</th>
							<th class="text-right">Actions</th>
						</tr>
					</thead>
					<tbody>
						{#each products as p}
							{@const product = p as {id: string; name: string; sku: string; barcode: string; category_name: string; brand_name: string; purchase_price: unknown; retail_price: unknown; total_stock: unknown; gst_rate: unknown; status: string; image_url: string; hsn_code: string}}
							<tr>
								<td>
									<div class="flex items-center gap-3">
										{#if product.image_url}
											<img src={product.image_url} alt={product.name} class="w-10 h-10 rounded-lg object-cover border border-slate-200" />
										{:else}
											<div class="w-10 h-10 rounded-lg bg-slate-100 flex items-center justify-center flex-shrink-0">
												<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#94a3b8" stroke-width="2">
													<rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="8.5" cy="8.5" r="1.5"/><polyline points="21 15 16 10 5 21"/>
												</svg>
											</div>
										{/if}
										<div>
											<div class="font-medium text-slate-900 text-sm">{product.name}</div>
											{#if product.hsn_code}<div class="text-xs text-slate-400">HSN: {product.hsn_code}</div>{/if}
										</div>
									</div>
								</td>
								<td>
									<code class="text-xs bg-slate-100 px-2 py-0.5 rounded font-mono">{product.sku}</code>
								</td>
								<td class="text-sm text-slate-600">{product.category_name ?? '—'}</td>
								<td class="text-right text-sm font-mono">{formatINR(product.purchase_price)}</td>
								<td class="text-right text-sm font-mono font-medium">{formatINR(product.retail_price)}</td>
								<td class="text-right">
									<span class="font-semibold text-sm {Number(product.total_stock ?? 0) <= 0 ? 'text-red-600' : Number(product.total_stock ?? 0) <= 10 ? 'text-amber-600' : 'text-green-700'}">
										{Number(product.total_stock ?? 0).toLocaleString('en-IN')}
									</span>
								</td>
								<td class="text-sm text-slate-600">{product.gst_rate}%</td>
								<td>
									<span class="badge {product.status === 'active' ? 'badge-success' : product.status === 'inactive' ? 'badge-warning' : 'badge-danger'}">
										{product.status}
									</span>
								</td>
								<td class="text-right">
									<div class="flex items-center justify-end gap-1">
										{#if canAdjustStock}
											<button class="btn btn-sm btn-secondary" title="Add stock received" on:click={() => openAddStock(product)}>+ Stock</button>
										{/if}
										<a href="/inventory/products/{product.id}" class="btn btn-sm btn-ghost" title="View">
											<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/><circle cx="12" cy="12" r="3"/></svg>
										</a>
										<a href="/inventory/products/{product.id}?edit=1" class="btn btn-sm btn-ghost" title="Edit">
											<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
										</a>
									</div>
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>

			<!-- Pagination -->
			{#if totalPages > 1}
				<div class="flex items-center justify-between px-4 py-3 border-t border-slate-100">
					<p class="text-sm text-slate-500">
						Showing {((page - 1) * perPage) + 1}–{Math.min(page * perPage, total)} of {total.toLocaleString('en-IN')}
					</p>
					<div class="flex gap-1">
						<button class="btn btn-sm btn-ghost" disabled={page <= 1} on:click={() => { page--; loadData(); }}>
							← Prev
						</button>
						{#each Array.from({length: Math.min(totalPages, 7)}, (_, i) => i + 1) as p}
							<button
								class="btn btn-sm {p === page ? 'btn-primary' : 'btn-ghost'}"
								on:click={() => { page = p; loadProducts(); }}
							>{p}</button>
						{/each}
						<button class="btn btn-sm btn-ghost" disabled={page >= totalPages} on:click={() => { page++; loadProducts(); }}>

							Next →
						</button>
					</div>
				</div>
			{/if}
		{/if}
	</div>
</div>

<!-- Add Product Modal -->
{#if showAddModal}
	<!-- svelte-ignore a11y_interactive_supports_focus -->
	<div
		class="modal-overlay"
		on:click|self={() => showAddModal = false}
		on:keydown={(e) => e.key === 'Escape' && (showAddModal = false)}
		role="dialog" tabindex="-1"
		aria-label="Add Product"
		aria-modal="true"
	>
		<div class="modal modal-lg">
			<div class="modal-header">
				<h2 class="modal-title">Add New Product</h2>
				<button class="btn btn-ghost btn-sm" on:click={() => showAddModal = false} aria-label="Close modal">
					<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
				</button>
			</div>
			<div class="modal-body">
				<form on:submit|preventDefault={createProduct}>
					<div class="grid grid-cols-2 gap-4">
						<div class="form-group">
							<label class="form-label required" for="prod-sku">SKU</label>
							<input id="prod-sku" type="text" class="form-control font-mono" bind:value={newProduct.sku} required placeholder="e.g. HNG-001" />
						</div>
						<div class="form-group">
							<label class="form-label" for="prod-barcode">Barcode (EAN/UPC)</label>
							<input id="prod-barcode" type="text" class="form-control font-mono" placeholder="Optional" />
						</div>
						<div class="form-group col-span-2">
							<label class="form-label required" for="prod-name">Product Name</label>
							<input id="prod-name" type="text" class="form-control" bind:value={newProduct.name} required placeholder="Full product name" />
						</div>
						<div class="form-group">
							<div class="flex items-center justify-between mb-1">
								<label class="form-label required mb-0" for="prod-category">Category</label>
								<button type="button" class="text-xs text-yellow-600 hover:text-yellow-700 font-semibold" on:click={() => showCatModal = true}>+ New</button>
							</div>
							<select id="prod-category" class="form-control" bind:value={newProduct.category_id} required>
								<option value="">{categories.length === 0 ? 'Loading...' : 'Select category'}</option>
								{#each categories as c}
									<option value={c.id}>{c.name}</option>
								{/each}
							</select>
						</div>
						<div class="form-group">
							<div class="flex items-center justify-between mb-1">
								<label class="form-label mb-0" for="prod-brand">Brand</label>
								<button type="button" class="text-xs text-yellow-600 hover:text-yellow-700 font-semibold" on:click={() => showBrandModal = true}>+ New</button>
							</div>
							<select id="prod-brand" class="form-control" bind:value={newProduct.brand_id}>
								<option value="">No brand</option>
								{#each brands as b}
									<option value={b.id}>{b.name}</option>
								{/each}
							</select>
						</div>
						<div class="form-group">
							<div class="flex items-center justify-between mb-1">
								<label class="form-label required mb-0" for="prod-unit">Unit of Measure</label>
								<button type="button" class="text-xs text-yellow-600 hover:text-yellow-700 font-semibold" on:click={() => showUnitModal = true}>+ New</button>
							</div>
							<select id="prod-unit" class="form-control" bind:value={newProduct.unit_id} required>
								<option value="">{units.length === 0 ? 'Loading...' : 'Select unit'}</option>
								{#each units as u}
									<option value={u.id}>{u.name} ({u.abbreviation})
</option>
								{/each}
							</select>
						</div>
						<div class="form-group">
							<label class="form-label" for="prod-hsn">HSN Code</label>
							<input id="prod-hsn" type="text" class="form-control" bind:value={newProduct.hsn_code} placeholder="e.g. 83024100" />
						</div>
						<div class="form-group">
							<label class="form-label required" for="prod-purchase-price">Purchase Price (₹)</label>
							<input id="prod-purchase-price" type="number" step="0.01" class="form-control" bind:value={newProduct.purchase_price} required min="0" />
						</div>
						<div class="form-group">
							<label class="form-label required" for="prod-retail-price">Retail Price (₹)</label>
							<input id="prod-retail-price" type="number" step="0.01" class="form-control" bind:value={newProduct.retail_price} required min="0" />
						</div>
						<div class="form-group">
							<label class="form-label required" for="prod-wholesale-price">Wholesale Price (₹)</label>
							<input id="prod-wholesale-price" type="number" step="0.01" class="form-control" bind:value={newProduct.wholesale_price} required min="0" />
						</div>
						<div class="form-group">
							<label class="form-label required" for="prod-gst">GST Rate (%)</label>
							<select id="prod-gst" class="form-control" bind:value={newProduct.gst_rate}>
								{#each GST_RATES as rate}
									<option value={rate}>{rate}%</option>
								{/each}
							</select>
						</div>
						<div class="form-group">
							<label class="form-label" for="prod-barcode">Barcode</label>
							<input id="prod-barcode" type="text" class="form-control font-mono" bind:value={newProduct.barcode} placeholder="Scan or type" />
						</div>
						<div class="form-group">
							<label class="form-label" for="prod-reorder">Reorder level</label>
							<input id="prod-reorder" type="number" min="0" class="form-control" bind:value={newProduct.reorder_threshold} />
						</div>
						<div class="form-group">
							<label class="form-label" for="prod-opening">Opening stock</label>
							<input id="prod-opening" type="number" min="0" step="any" class="form-control" bind:value={newProduct.opening_stock} placeholder="0" />
						</div>
					</div>

					<div class="modal-footer">
						<button type="button" class="btn btn-secondary" on:click={() => showAddModal = false}>Cancel</button>
						<button type="submit" class="btn btn-primary" disabled={submitting}>
							{#if submitting}
								<div class="spinner spinner-sm mr-2"></div>Saving…
							{:else}
								Create Product
							{/if}
						</button>
					</div>
				</form>
			</div>
		</div>
	</div>
{/if}

<!-- Quick-Create Category Modal -->
{#if showCatModal}
<div class="modal-overlay" on:click|self={() => showCatModal = false} on:keydown={(e) => e.key === 'Escape' && (showCatModal = false)} role="dialog" tabindex="-1" aria-modal="true">
	<div class="modal" style="max-width:360px;">
		<div class="modal-header"><h2 class="modal-title">New Category</h2>
			<button class="btn btn-ghost btn-sm" on:click={() => showCatModal = false}>✕</button></div>
		<div class="modal-body">
			<div class="form-group"><label class="form-label" for="products-f1">Category Name *</label>
				<!-- svelte-ignore a11y_autofocus -->
				<input id="products-f1" class="form-control" bind:value={newCatName} placeholder="e.g. Hinges" autofocus /></div>
		</div>
		<div class="modal-footer">
			<button class="btn btn-secondary" on:click={() => showCatModal = false}>Cancel</button>
			<button class="btn btn-primary" disabled={savingCat || !newCatName.trim()} on:click={createCategory}>
				{savingCat ? 'Creating...' : 'Create'}
			</button>
		</div>
	</div>
</div>
{/if}

<!-- Quick-Create Brand Modal -->
{#if showBrandModal}
<div class="modal-overlay" on:click|self={() => showBrandModal = false} on:keydown={(e) => e.key === 'Escape' && (showBrandModal = false)} role="dialog" tabindex="-1" aria-modal="true">
	<div class="modal" style="max-width:360px;">
		<div class="modal-header"><h2 class="modal-title">New Brand</h2>
			<button class="btn btn-ghost btn-sm" on:click={() => showBrandModal = false}>✕</button></div>
		<div class="modal-body">
			<div class="form-group"><label class="form-label" for="products-f2">Brand Name *</label>
				<!-- svelte-ignore a11y_autofocus -->
				<input id="products-f2" class="form-control" bind:value={newBrandName} placeholder="e.g. Hafele" autofocus /></div>
		</div>
		<div class="modal-footer">
			<button class="btn btn-secondary" on:click={() => showBrandModal = false}>Cancel</button>
			<button class="btn btn-primary" disabled={savingBrand || !newBrandName.trim()} on:click={createBrand}>
				{savingBrand ? 'Creating...' : 'Create'}
			</button>
		</div>
	</div>
</div>
{/if}

<!-- Quick-Create Unit Modal -->
{#if showUnitModal}
<div class="modal-overlay" on:click|self={() => showUnitModal = false} on:keydown={(e) => e.key === 'Escape' && (showUnitModal = false)} role="dialog" tabindex="-1" aria-modal="true">
	<div class="modal" style="max-width:360px;">
		<div class="modal-header"><h2 class="modal-title">New Unit of Measure</h2>
			<button class="btn btn-ghost btn-sm" on:click={() => showUnitModal = false}>✕</button></div>
		<div class="modal-body">
			<div class="form-group"><label class="form-label" for="products-f3">Name *</label>
				<!-- svelte-ignore a11y_autofocus -->
				<input id="products-f3" class="form-control" bind:value={newUnitName} placeholder="e.g. Piece" autofocus /></div>
			<div class="form-group"><label class="form-label" for="products-f4">Abbreviation *</label>
				<input id="products-f4" class="form-control" bind:value={newUnitAbbr} placeholder="e.g. pcs" /></div>
		</div>
		<div class="modal-footer">
			<button class="btn btn-secondary" on:click={() => showUnitModal = false}>Cancel</button>
			<button class="btn btn-primary" disabled={savingUnit || !newUnitName.trim()} on:click={createUnit}>
				{savingUnit ? 'Creating...' : 'Create'}
			</button>
		</div>
	</div>
</div>
{/if}

{#if stockFor}
	<div class="modal-overlay" role="presentation" on:click|self={() => (stockFor = null)}>
		<form class="modal" style="max-width:440px" on:submit|preventDefault={saveAddStock}>
			<div class="modal-header">
				<div>
					<h2 class="modal-title">Add stock</h2>
					<p class="text-sm text-slate-500">{stockFor.name} · current stock {Number(stockFor.total_stock ?? 0).toLocaleString('en-IN')}</p>
				</div>
				<button type="button" class="modal-close" on:click={() => (stockFor = null)} aria-label="Close">×</button>
			</div>
			<div class="modal-body space-y-4">
				<div class="form-group">
					<label class="form-label required" for="as-qty">Quantity received</label>
					<!-- svelte-ignore a11y_autofocus -->
					<input id="as-qty" class="form-control" type="number" min="0" step="any" bind:value={stockForm.quantity} required autofocus />
				</div>
				<div class="form-group">
					<label class="form-label" for="as-rack">Put in rack</label>
					<select id="as-rack" class="form-control" bind:value={stockForm.location_id}>
						<option value="">Main location</option>
						{#each stockRacks as r}<option value={r.id}>{r.label ?? r.rack_code}</option>{/each}
					</select>
				</div>
				<div class="form-group">
					<label class="form-label" for="as-note">Note</label>
					<input id="as-note" class="form-control" bind:value={stockForm.reason} placeholder="Supplier, bill no…" />
				</div>
				<p class="text-xs text-slate-400">For supplier bills you also need to pay, use Purchases → New Purchase Order so the amount is tracked.</p>
			</div>
			<div class="modal-footer">
				<button type="button" class="btn btn-secondary" on:click={() => (stockFor = null)}>Cancel</button>
				<button type="submit" class="btn btn-primary" disabled={savingStock}>{savingStock ? 'Saving…' : 'Add to stock'}</button>
			</div>
		</form>
	</div>
{/if}

<style>
	.modal-lg { max-width: 640px; }
</style>
