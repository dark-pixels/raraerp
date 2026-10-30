<script lang="ts">
	import { onMount } from 'svelte';
	import { inventoryApi, rackApi, errorMessage } from '$api/client';
	import { toasts } from '$stores/toasts';
	import { formatDateTime, humanize, downloadCsv, STATUS_BADGE } from '$utils/format';

	type Row = Record<string, any>;

	let imports: Row[] = [];
	let racks: Row[] = [];
	let loading = true;
	let uploading = false;
	let file: File | null = null;
	let mode = 'create';
	let locationId = '';
	let fileInput: HTMLInputElement;
	let result: Row | null = null;
	let errorsFor: Row | null = null;

	const MODES: Record<string, string> = {
		create: 'Adds new products. Rows whose SKU already exists are reported as errors. A quantity column sets opening stock.',
		update: 'Updates the listed columns for existing SKUs. Empty cells leave the current value unchanged.',
		opening_stock: 'Adds the quantity to stock for existing SKUs (use once, when you start using the system).',
		adjustment: 'Adds positive quantities or removes negative quantities for existing SKUs.'
	};

	async function load() {
		loading = true;
		try {
			const [res, r]: any[] = await Promise.all([inventoryApi.imports.list({ per_page: 30 }), rackApi.racks().catch(() => ({ data: [] }))]);
			imports = res.data ?? [];
			racks = r.data ?? [];
		} catch (e) { toasts.error('Failed to load import history', errorMessage(e)); }
		loading = false;
	}

	async function upload() {
		if (!file) { toasts.error('Select a file first'); return; }
		uploading = true;
		result = null;
		try {
			result = (await inventoryApi.imports.upload(file, mode, locationId || undefined)) as Row;
			const r = result!;
			if (r.error_count > 0) toasts.warning(r.message, `${r.error_count} rows had errors`);
			else toasts.success(r.message);
			file = null;
			if (fileInput) fileInput.value = '';
			await load();
		} catch (e) { toasts.error('Import failed', errorMessage(e)); }
		uploading = false;
	}

	async function showErrors(id: string) {
		try { errorsFor = (await inventoryApi.imports.get(id)) as Row; }
		catch (e) { toasts.error('Failed to load errors', errorMessage(e)); }
	}

	function downloadTemplate() {
		const sample = mode === 'create'
			? [{ sku: 'HNG-001', name: 'Soft-close hinge 35mm', category: 'Hinges', brand: 'Hettich', unit: 'Pcs', barcode: '', hsn_code: '8302', gst_rate: 18, purchase_price: 45, retail_price: 75, wholesale_price: 62, reorder_threshold: 20, quantity: 100, rack_code: '' }]
			: mode === 'update'
				? [{ sku: 'HNG-001', name: '', retail_price: 79, wholesale_price: 65, gst_rate: '', reorder_threshold: '' }]
				: [{ sku: 'HNG-001', quantity: mode === 'adjustment' ? -2 : 100, rack_code: '' }];
		downloadCsv(`import-template-${mode}.csv`, sample);
	}

	onMount(load);
</script>

<svelte:head><title>Bulk Import — RARA Enterprise ERP</title></svelte:head>

<div class="page-container">
	<div class="page-header">
		<h1 class="page-title">Bulk Import</h1>
		<p class="page-subtitle">Create or update products and stock from a CSV or Excel (.xlsx) file</p>
	</div>

	<div class="grid grid-cols-1 lg:grid-cols-2 gap-6 mb-6">
		<div class="card">
			<div class="card-header"><h2 class="text-lg font-semibold">Upload file</h2></div>
			<div class="card-body space-y-4">
				<div class="form-group">
					<label class="form-label" for="imp-mode">Import mode</label>
					<select id="imp-mode" class="input" bind:value={mode}>
						<option value="create">Create new products</option>
						<option value="update">Update existing products</option>
						<option value="opening_stock">Opening stock</option>
						<option value="adjustment">Stock adjustment</option>
					</select>
					<span class="form-hint">{MODES[mode]}</span>
				</div>
				{#if mode !== 'update'}
					<div class="form-group">
						<label class="form-label" for="imp-loc">Default rack (when a row has no rack_code)</label>
						<select id="imp-loc" class="input" bind:value={locationId}>
							<option value="">Main location</option>
							{#each racks as r}<option value={r.id}>{r.label ?? r.rack_code}</option>{/each}
						</select>
					</div>
				{/if}
				<div class="form-group">
					<label class="form-label" for="imp-file">File</label>
					<input id="imp-file" class="input" type="file" accept=".csv,.xlsx"
						bind:this={fileInput}
						on:change={(e) => (file = (e.target as HTMLInputElement).files?.[0] ?? null)} />
					{#if file}<span class="form-hint">{file.name} · {(file.size / 1024).toFixed(1)} KB</span>{/if}
				</div>
				<button class="btn btn-primary w-full" disabled={uploading || !file} on:click={upload}>
					{uploading ? 'Importing…' : 'Upload & import'}
				</button>

				{#if result}
					<div class="rounded-xl border {result.error_count > 0 ? 'border-amber-200 bg-amber-50' : 'border-green-200 bg-green-50'} p-4 text-sm">
						<p class="font-semibold">{result.message}</p>
						{#if result.errors?.length}
							<ul class="mt-2 space-y-1 text-amber-800 max-h-48 overflow-auto">
								{#each result.errors as err}<li>{err}</li>{/each}
							</ul>
						{/if}
					</div>
				{/if}
			</div>
		</div>

		<div class="card">
			<div class="card-header flex items-center justify-between">
				<h2 class="text-lg font-semibold">File format</h2>
				<button class="btn btn-secondary btn-sm" on:click={downloadTemplate}>Download template</button>
			</div>
			<div class="card-body text-sm text-slate-600 space-y-3">
				<p>Row 1 must be a header row. Columns are matched by name, in any order:</p>
				<div class="bg-slate-50 rounded-lg p-3 font-mono text-xs overflow-x-auto">
					sku*, name, category, brand, unit, barcode, hsn_code, gst_rate, purchase_price, retail_price, wholesale_price, reorder_threshold, quantity, rack_code
				</div>
				<ul class="list-disc pl-5 space-y-1 text-xs text-slate-500">
					<li><code>sku</code> is always required; <code>name</code> is required when creating.</li>
					<li>Unknown categories and brands are created automatically.</li>
					<li>Common alternatives are understood, e.g. "Product Name", "MRP", "Qty", "HSN".</li>
					<li>Each row is processed independently — one bad row never blocks the rest.</li>
				</ul>
			</div>
		</div>
	</div>

	<div class="card">
		<div class="card-header"><h2 class="text-lg font-semibold">Import history</h2></div>
		{#if loading}
			<div class="card-body flex justify-center py-8"><div class="spinner"></div></div>
		{:else if imports.length === 0}
			<div class="card-body text-center py-8 text-slate-400">No imports yet</div>
		{:else}
			<div class="table-container">
				<table class="table">
					<thead><tr><th>File</th><th>Mode</th><th>Status</th><th class="text-right">Rows</th><th class="text-right">Imported</th><th class="text-right">Errors</th><th>When</th><th></th></tr></thead>
					<tbody>
					{#each imports as imp}
						<tr>
							<td class="font-medium text-sm">{imp.filename}{#if imp.created_by_name}<div class="text-xs text-slate-400">{imp.created_by_name}</div>{/if}</td>
							<td><span class="badge badge-secondary">{humanize(imp.mode)}</span></td>
							<td><span class="badge {STATUS_BADGE[imp.status] ?? 'badge-secondary'}">{imp.status}</span></td>
							<td class="text-right">{imp.total_rows}</td>
							<td class="text-right text-green-600 font-medium">{imp.success_count}</td>
							<td class="text-right text-red-600 font-medium">{imp.error_count}</td>
							<td class="text-sm text-slate-500">{formatDateTime(imp.created_at)}</td>
							<td>{#if imp.error_count > 0}<button class="btn btn-ghost btn-sm" on:click={() => showErrors(imp.id)}>View errors</button>{/if}</td>
						</tr>
					{/each}
					</tbody>
				</table>
			</div>
		{/if}
	</div>
</div>

{#if errorsFor}
	<div class="modal-overlay" on:click|self={() => (errorsFor = null)} on:keydown={(e) => e.key === 'Escape' && (errorsFor = null)} role="dialog" aria-modal="true" tabindex="-1" aria-label="Import errors">
		<div class="modal modal-lg">
			<div class="modal-header">
				<h2 class="modal-title">Errors — {errorsFor.filename}</h2>
				<button class="modal-close" on:click={() => (errorsFor = null)} aria-label="Close">×</button>
			</div>
			<div class="modal-body">
				<table class="table">
					<thead><tr><th>Row</th><th>Error</th><th>Data</th></tr></thead>
					<tbody>
						{#each errorsFor.errors ?? [] as e}
							<tr><td>{e.row}</td><td class="text-red-700 text-sm">{e.message}</td><td class="font-mono text-xs text-slate-500 truncate" style="max-width:280px">{e.data}</td></tr>
						{/each}
					</tbody>
				</table>
			</div>
		</div>
	</div>
{/if}
