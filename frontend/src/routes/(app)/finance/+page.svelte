<script lang="ts">
	import { onMount } from 'svelte';
	import { financeApi } from '$lib/api/client';
	import { toasts } from '$stores/toasts';

	let payables: Record<string, unknown>[] = [];
	let receivables: Record<string, unknown>[] = [];
	let expenses: Record<string, unknown>[] = [];
	let summary: Record<string, unknown> = {};
	let loading = true;
	let activeTab = 'overview';

	const formatINR = (v: unknown) =>
		new Intl.NumberFormat('en-IN', { style: 'currency', currency: 'INR', maximumFractionDigits: 0 }).format(Number(v ?? 0));
	const formatDate = (d: unknown) => d ? new Date(String(d)).toLocaleDateString('en-IN') : '—';

	async function load() {
		loading = true;
		try {
			const [p, r, e]: any[] = await Promise.all([
				financeApi.payables({ limit: 50 }),
				financeApi.receivables({ limit: 50 }),
				financeApi.expenses({ limit: 50 })
			]);
			payables = p.data ?? p ?? [];
			receivables = r.data ?? r ?? [];
			expenses = e.data ?? e ?? [];
		} catch { toasts.error('Failed to load finance data'); }
		loading = false;
	}

	onMount(load);
</script>

<svelte:head><title>Finance — RARA Enterprise ERP</title></svelte:head>

<div class="page-container">
	<div class="page-header">
		<h1 class="page-title">Finance</h1>
	</div>

	<div class="flex gap-2 mb-6">
		{#each ['overview', 'payables', 'receivables', 'expenses'] as tab}
			<button class="btn {activeTab === tab ? 'btn-primary' : 'btn-secondary'}" on:click={() => activeTab = tab}>
				{tab.charAt(0).toUpperCase() + tab.slice(1)}
			</button>
		{/each}
	</div>

	{#if loading}
		<div class="flex items-center justify-center py-16 text-slate-400">Loading...</div>
	{:else if activeTab === 'overview'}
		<div class="grid grid-cols-3 gap-4 mb-6">
			<div class="card"><div class="card-body">
				<p class="text-xs font-semibold uppercase tracking-widest text-red-500 mb-1">Total Payables</p>
				<p class="text-2xl font-bold text-red-600">
					{formatINR(payables.reduce((s, p) => s + Number(p.outstanding_balance ?? p.outstanding_amount ?? 0), 0))}
				</p>
				<p class="text-sm text-slate-500 mt-1">{payables.length} suppliers outstanding</p>
			</div></div>
			<div class="card"><div class="card-body">
				<p class="text-xs font-semibold uppercase tracking-widest text-green-500 mb-1">Total Receivables</p>
				<p class="text-2xl font-bold text-green-600">
					{formatINR(receivables.reduce((s, r) => s + Number(r.outstanding_amount ?? r.outstanding_balance ?? 0), 0))}
				</p>
				<p class="text-sm text-slate-500 mt-1">{receivables.length} customers outstanding</p>
			</div></div>
			<div class="card"><div class="card-body">
				<p class="text-xs font-semibold uppercase tracking-widest text-yellow-500 mb-1">Total Expenses</p>
				<p class="text-2xl font-bold text-yellow-600">
					{formatINR(expenses.reduce((s, e) => s + Number(e.amount ?? 0), 0))}
				</p>
				<p class="text-sm text-slate-500 mt-1">{expenses.length} expense entries</p>
			</div></div>
		</div>
	{:else if activeTab === 'payables'}
		<div class="card">
			<div class="table-container">
				<table class="table">
					<thead><tr><th>Supplier</th><th>Outstanding</th><th>Due Date</th><th>Status</th></tr></thead>
					<tbody>
					{#each payables as p}
						<tr>
							<td class="font-medium">{p.supplier_name ?? p.business_name ?? '—'}</td>
							<td class="font-semibold text-red-600">{formatINR(p.outstanding_amount ?? p.outstanding_balance)}</td>
							<td class="text-sm text-slate-600">{formatDate(p.due_date)}</td>
							<td><span class="badge badge-warning">{p.payment_status ?? p.status ?? '—'}</span></td>
						</tr>
					{/each}
					{#if payables.length === 0}<tr><td colspan="4" class="text-center text-slate-400 py-8">No payables outstanding</td></tr>{/if}
					</tbody>
				</table>
			</div>
		</div>
	{:else if activeTab === 'receivables'}
		<div class="card">
			<div class="table-container">
				<table class="table">
					<thead><tr><th>Customer / Invoice</th><th>Outstanding</th><th>Due Date</th><th>Status</th></tr></thead>
					<tbody>
					{#each receivables as r}
						<tr>
							<td class="font-medium">{r.customer_name ?? r.invoice_number ?? '—'}</td>
							<td class="font-semibold text-green-600">{formatINR(r.outstanding_amount ?? r.outstanding_balance)}</td>
							<td class="text-sm text-slate-600">{formatDate(r.due_date)}</td>
							<td><span class="badge badge-warning">{r.status ?? '—'}</span></td>
						</tr>
					{/each}
					{#if receivables.length === 0}<tr><td colspan="4" class="text-center text-slate-400 py-8">No receivables outstanding</td></tr>{/if}
					</tbody>
				</table>
			</div>
		</div>
	{:else if activeTab === 'expenses'}
		<div class="card">
			<div class="table-container">
				<table class="table">
					<thead><tr><th>Date</th><th>Category</th><th>Amount</th><th>Notes</th></tr></thead>
					<tbody>
					{#each expenses as e}
						<tr>
							<td class="text-sm">{formatDate(e.expense_date ?? e.created_at)}</td>
							<td>{e.category ?? '—'}</td>
							<td class="font-semibold text-yellow-700">{formatINR(e.amount)}</td>
							<td class="text-slate-500 text-sm">{e.notes ?? '—'}</td>
						</tr>
					{/each}
					{#if expenses.length === 0}<tr><td colspan="4" class="text-center text-slate-400 py-8">No expense records</td></tr>{/if}
					</tbody>
				</table>
			</div>
		</div>
	{/if}
</div>
