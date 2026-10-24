<script lang="ts">
	import { onMount } from 'svelte';
	import { inventoryApi } from '$lib/api/client';
	import { toasts } from '$stores/toasts';

	let imports: Record<string, unknown>[] = [];
	let loading = true;
	let uploading = false;
	let file: File | null = null;
	let mode = 'create';
	let fileInput: HTMLInputElement;

	const formatDate = (d: unknown) => d ? new Date(String(d)).toLocaleString('en-IN') : '—';
	const STATUS_COLORS: Record<string, string> = { pending: 'badge-warning', processing: 'badge-info', completed: 'badge-success', failed: 'badge-danger' };

	async function load() {
		loading = true;
		try {
			const res: any = await inventoryApi.imports.list({ limit: 20 });
			imports = res.data ?? res ?? [];
		} catch { toasts.error('Failed to load import history'); }
		loading = false;
	}

	async function upload() {
		if (!file) { toasts.error('Select a file first'); return; }
		uploading = true;
		try {
			await inventoryApi.imports.upload(file, mode);
			toasts.success('File uploaded — processing started');
			file = null;
			if (fileInput) fileInput.value = '';
			await load();
		} catch (e: any) { toasts.error(e.message ?? 'Upload failed'); }
		uploading = false;
	}

	onMount(load);
</script>

<svelte:head><title>Bulk Import — RARA Enterprise ERP</title></svelte:head>

<div class="page-container">
	<div class="page-header">
		<h1 class="page-title">Bulk Import</h1>
	</div>

	<div class="grid grid-cols-2 gap-6 mb-6">
		<div class="card">
			<div class="card-header"><h2 class="text-lg font-semibold">Upload File</h2></div>
			<div class="card-body">
				<div class="form-group">
					<label class="form-label">Import Mode</label>
					<select class="input" bind:value={mode}>
						<option value="create">Create New Products</option>
						<option value="update">Update Existing</option>
						<option value="opening_stock">Opening Stock</option>
						<option value="adjustment">Stock Adjustment</option>
					</select>
				</div>
				<div class="form-group">
					<label class="form-label">File (CSV / Excel)</label>
					<input class="input" type="file" accept=".csv,.xlsx,.xls"
						bind:this={fileInput}
						on:change={(e) => file = (e.target as HTMLInputElement).files?.[0] ?? null} />
				</div>
				{#if file}
					<p class="text-sm text-slate-500 mb-3">Selected: <strong>{file.name}</strong> ({(file.size/1024).toFixed(1)} KB)</p>
				{/if}
				<button class="btn btn-primary w-full" disabled={uploading || !file} on:click={upload}>
					{uploading ? 'Uploading...' : '⬆ Upload & Process'}
				</button>
			</div>
		</div>

		<div class="card">
			<div class="card-header"><h2 class="text-lg font-semibold">Template Format</h2></div>
			<div class="card-body">
				<p class="text-sm text-slate-600 mb-3">CSV columns required:</p>
				<div class="bg-slate-50 rounded-lg p-3 font-mono text-xs text-slate-600 overflow-x-auto">
					sku, name, category, unit, purchase_price, retail_price, wholesale_price, gst_rate, hsn_code, reorder_threshold
				</div>
				<p class="text-xs text-slate-400 mt-3">Row 1 must be the header. All columns except <code>sku</code> and <code>name</code> are optional.</p>
			</div>
		</div>
	</div>

	<div class="card">
		<div class="card-header"><h2 class="text-lg font-semibold">Import History</h2></div>
		{#if loading}
			<div class="card-body text-center py-8 text-slate-400">Loading...</div>
		{:else if imports.length === 0}
			<div class="card-body text-center py-8 text-slate-400">No imports yet</div>
		{:else}
			<div class="table-container">
				<table class="table">
					<thead><tr><th>File</th><th>Mode</th><th>Status</th><th>Total</th><th>Success</th><th>Errors</th><th>Time</th></tr></thead>
					<tbody>
					{#each imports as imp}
						<tr>
							<td class="font-medium text-sm">{imp.filename}</td>
							<td><span class="badge badge-secondary">{imp.mode}</span></td>
							<td><span class="badge {STATUS_COLORS[String(imp.status)] ?? 'badge-secondary'}">{imp.status}</span></td>
							<td>{imp.total_rows}</td>
							<td class="text-green-600 font-medium">{imp.success_count}</td>
							<td class="text-red-600 font-medium">{imp.error_count}</td>
							<td class="text-sm text-slate-500">{formatDate(imp.created_at)}</td>
						</tr>
					{/each}
					</tbody>
				</table>
			</div>
		{/if}
	</div>
</div>
