<script lang="ts">
	import { onMount } from 'svelte';
	import { auditApi } from '$lib/api/client';
	import { toasts } from '$stores/toasts';

	let logs: Record<string, unknown>[] = [];
	let loading = true;
	const formatDate = (d: unknown) => d ? new Date(String(d)).toLocaleString('en-IN') : '—';

	const ACTION_COLORS: Record<string, string> = { create: 'badge-success', update: 'badge-info', delete: 'badge-danger', login: 'badge-primary', logout: 'badge-secondary' };

	async function load() {
		loading = true;
		try {
			const res: any = await auditApi.list({ limit: 100 });
			logs = res.data ?? res ?? [];
		} catch { toasts.error('Failed to load audit logs'); }
		loading = false;
	}

	onMount(load);
</script>

<svelte:head><title>Audit Logs — RARA Enterprise ERP</title></svelte:head>

<div class="page-container">
	<div class="page-header">
		<h1 class="page-title">Audit Logs</h1>
	</div>

	{#if loading}
		<div class="flex items-center justify-center py-16 text-slate-400">Loading...</div>
	{:else if logs.length === 0}
		<div class="card"><div class="card-body text-center py-16">
			<p class="text-slate-400 text-lg">No audit logs yet</p>
		</div></div>
	{:else}
		<div class="card">
			<div class="table-container">
				<table class="table">
					<thead><tr><th>Time</th><th>User</th><th>Action</th><th>Entity</th><th>Details</th></tr></thead>
					<tbody>
					{#each logs as log}
						<tr>
							<td class="text-sm text-slate-500 whitespace-nowrap">{formatDate(log.created_at)}</td>
							<td class="font-medium">{log.user_email ?? log.user_id ?? '—'}</td>
							<td><span class="badge {ACTION_COLORS[String(log.action)] ?? 'badge-secondary'}">{log.action}</span></td>
							<td class="text-sm">{log.entity_type ?? '—'}</td>
							<td class="text-xs text-slate-400 max-w-xs truncate">{log.description ?? log.entity_id ?? '—'}</td>
						</tr>
					{/each}
					</tbody>
				</table>
			</div>
		</div>
	{/if}
</div>
