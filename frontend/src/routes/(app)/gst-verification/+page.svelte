<script lang="ts">
	import { gstApi } from '$lib/api/client';
	import { toasts } from '$stores/toasts';
	import { onMount } from 'svelte';

	let gstin = '';
	let verifying = false;
	let result: Record<string, unknown> | null = null;
	let history: Record<string, unknown>[] = [];
	let loadingHistory = true;

	const formatDate = (d: unknown) => d ? new Date(String(d)).toLocaleDateString('en-IN') : '—';

	async function verify() {
		if (!gstin.trim() || gstin.length !== 15) { toasts.error('Enter a valid 15-character GSTIN'); return; }
		verifying = true; result = null;
		try {
			result = await gstApi.verify(gstin.toUpperCase()) as Record<string, unknown>;
			toasts.success('GSTIN verified');
			await loadHistory();
		} catch (e: any) { toasts.error(e.message ?? 'Verification failed'); }
		verifying = false;
	}

	async function loadHistory() {
		try {
			const res: any = await gstApi.history();
			history = res.data ?? res ?? [];
		} catch {}
		loadingHistory = false;
	}

	onMount(loadHistory);
</script>

<svelte:head><title>GST Verification — RARA Enterprise ERP</title></svelte:head>

<div class="page-container">
	<div class="page-header">
		<div>
			<h1 class="page-title">GST Verification</h1>
			<p class="page-subtitle">Verify supplier and customer GSTINs</p>
		</div>
	</div>

	<div class="grid grid-cols-2 gap-6">
		<div>
			<div class="card mb-4">
				<div class="card-body">
					<h2 class="text-lg font-semibold text-slate-700 mb-4">Verify GSTIN</h2>
					<div class="form-group">
						<label class="form-label" for="gst-verification-f1">GSTIN (15 characters)</label>
						<input id="gst-verification-f1" class="input" bind:value={gstin} placeholder="e.g. 27AAAPL1234C1ZV" maxlength="15"
							style="font-family: monospace; text-transform: uppercase;"
							on:input={() => gstin = gstin.toUpperCase()} />
					</div>
					<button class="btn btn-primary w-full" disabled={verifying || gstin.length !== 15} on:click={verify}>
						{verifying ? 'Verifying...' : 'Verify GSTIN'}
					</button>
				</div>
			</div>

			{#if result}
				<div class="card">
					<div class="card-body">
						<div class="flex items-center gap-2 mb-4">
							<span class="badge badge-{result.status === 'active' ? 'success' : result.status === 'unknown' || result.status === 'pending' ? 'warning' : 'danger'} text-sm px-3 py-1">
								{result.status === 'active' ? '✓ Active' : result.status === 'unknown' || result.status === 'pending' ? 'Not verified' : `✗ ${String(result.status)}`}
							</span>
							{#if result.message}<span class="text-sm text-slate-500">{result.message}</span>{/if}
						</div>
						{#if result.trade_name || result.legal_name}
							<div class="mb-2"><span class="text-sm text-slate-500">Business:</span>
								<span class="font-semibold ml-2">{result.trade_name ?? result.legal_name}</span></div>
						{/if}
						{#if result.gstin}<div class="mb-2"><span class="text-sm text-slate-500">GSTIN:</span>
							<span class="font-mono ml-2">{result.gstin}</span></div>{/if}
						{#if result.registration_date}<div class="mb-2"><span class="text-sm text-slate-500">Registered:</span>
							<span class="ml-2">{formatDate(result.registration_date)}</span></div>{/if}
						{#if result.state}<div class="mb-2"><span class="text-sm text-slate-500">State:</span>
							<span class="ml-2">{result.state}</span></div>{/if}
					</div>
				</div>
			{/if}
		</div>

		<div class="card">
			<div class="card-header"><h2 class="text-lg font-semibold">Verification History</h2></div>
			<div class="table-container">
				<table class="table">
					<thead><tr><th>GSTIN</th><th>Status</th><th>Date</th></tr></thead>
					<tbody>
					{#if loadingHistory}
						<tr><td colspan="3" class="text-center py-8 text-slate-400">Loading...</td></tr>
					{:else if history.length === 0}
						<tr><td colspan="3" class="text-center py-8 text-slate-400">No verifications yet</td></tr>
					{:else}
					{#each history as h}
						<tr>
							<td class="font-mono text-sm">{h.gstin}</td>
							<td><span class="badge badge-{h.status === 'active' ? 'success' : 'danger'}">{h.status}</span></td>
							<td class="text-sm text-slate-500">{formatDate(h.verified_at ?? h.created_at)}</td>
						</tr>
					{/each}
					{/if}
					</tbody>
				</table>
			</div>
		</div>
	</div>
</div>
