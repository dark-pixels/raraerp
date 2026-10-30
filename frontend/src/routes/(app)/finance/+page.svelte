<script lang="ts">
	import { onMount } from 'svelte';
	import { financeApi, errorMessage } from '$api/client';
	import { currentUser, hasPermission } from '$stores/auth';
	import { toasts } from '$stores/toasts';
	import { formatINR, formatDate, humanize, isoDate, toNumber, STATUS_BADGE } from '$utils/format';

	type Row = Record<string, any>;

	let summary: Row = {};
	let payables: Row[] = [];
	let receivables: Row[] = [];
	let expenses: Row[] = [];
	let loading = true;
	let activeTab: 'overview' | 'payables' | 'receivables' | 'expenses' = 'overview';
	let from = isoDate(new Date(new Date().getFullYear(), new Date().getMonth(), 1));
	let to = isoDate();

	let showExpense = false;
	let saving = false;
	let editExpenseId: string | null = null;
	const blankExpense = () => ({ category: '', amount: '', date: isoDate(), payment_method: 'cash', reference: '', description: '' });
	let expense = blankExpense();
	const EXPENSE_CATEGORIES = ['Rent', 'Salaries', 'Electricity', 'Transport', 'Packaging', 'Maintenance', 'Marketing', 'Office supplies', 'Bank charges', 'Miscellaneous'];

	let dueEdit: { kind: 'invoice' | 'purchase'; id: string; label: string; due_date: string; min: string } | null = null;

	$: canApprove = hasPermission($currentUser, 'payments.approve');
	// Approvers can change any expense; others only their own while it is pending.
	$: canEditExpense = (e: Row) => canApprove || (e.approval_status === 'pending' && e.created_by_id === $currentUser?.id);

	async function loadSummary() {
		try { summary = (await financeApi.summary({ from, to })) as Row; }
		catch (e) { toasts.error('Failed to load summary', errorMessage(e)); }
	}

	async function load() {
		loading = true;
		try {
			const [p, r, e]: any[] = await Promise.all([
				financeApi.payables(),
				financeApi.receivables(),
				financeApi.expenses({ per_page: 200 }),
				loadSummary()
			]);
			payables = p.data ?? [];
			receivables = r.data ?? [];
			expenses = e.data ?? [];
		} catch (e) {
			toasts.error('Failed to load finance data', errorMessage(e));
		}
		loading = false;
	}

	function openNewExpense() {
		editExpenseId = null;
		expense = blankExpense();
		showExpense = true;
	}

	function openEditExpense(e: Row) {
		editExpenseId = e.id;
		expense = {
			category: e.category ?? '',
			amount: String(e.amount ?? ''),
			date: String(e.date ?? isoDate()).slice(0, 10),
			payment_method: e.payment_method ?? 'cash',
			reference: e.reference ?? '',
			description: e.description ?? ''
		};
		showExpense = true;
	}

	function closeExpense() {
		showExpense = false;
		editExpenseId = null;
	}

	async function saveExpense() {
		saving = true;
		try {
			if (editExpenseId) {
				await financeApi.updateExpense(editExpenseId, expense);
				toasts.success('Expense updated');
			} else {
				const res: any = await financeApi.createExpense(expense);
				toasts.success('Expense recorded', res.approval_status === 'pending' ? 'Awaiting approval' : undefined);
			}
			closeExpense();
			expense = blankExpense();
			await load();
		} catch (e) { toasts.error('Could not save expense', errorMessage(e)); }
		saving = false;
	}

	async function removeExpense(e: Row) {
		if (!confirm(`Delete the ${e.category} expense of ${formatINR(e.amount)}? This cannot be undone.`)) return;
		try { await financeApi.deleteExpense(e.id); toasts.success('Expense deleted'); await load(); }
		catch (err) { toasts.error('Could not delete expense', errorMessage(err)); }
	}

	function openDueDate(kind: 'invoice' | 'purchase', r: Row) {
		dueEdit = {
			kind,
			id: r.id,
			label: kind === 'invoice' ? r.invoice_number : r.po_number,
			due_date: r.due_date ? String(r.due_date).slice(0, 10) : isoDate(),
			min: String((kind === 'invoice' ? r.invoice_date : r.purchase_date) ?? '').slice(0, 10)
		};
	}

	async function saveDueDate() {
		if (!dueEdit) return;
		saving = true;
		try {
			await financeApi.updateDueDate(dueEdit.kind, dueEdit.id, dueEdit.due_date);
			toasts.success('Due date updated');
			dueEdit = null;
			await load();
		} catch (e) { toasts.error('Could not update due date', errorMessage(e)); }
		saving = false;
	}

	async function review(id: string, decision: 'approve' | 'reject') {
		try { await financeApi.reviewExpense(id, decision); toasts.success(`Expense ${decision}d`); await load(); }
		catch (e) { toasts.error('Action failed', errorMessage(e)); }
	}

	onMount(load);
</script>

<svelte:head><title>Finance — RARA Enterprise ERP</title></svelte:head>

<div class="page-container">
	<div class="page-header flex items-center justify-between gap-4 flex-wrap">
		<div>
			<h1 class="page-title">Finance</h1>
			<p class="page-subtitle">Cash flow, dues and expenses</p>
		</div>
		<button class="btn btn-primary" on:click={openNewExpense}>+ Record expense</button>
	</div>

	<div class="flex gap-2 mb-6 flex-wrap" role="tablist">
		{#each [['overview', 'Overview'], ['receivables', `Receivables (${receivables.length})`], ['payables', `Payables (${payables.length})`], ['expenses', 'Expenses']] as [key, label]}
			<button role="tab" aria-selected={activeTab === key} class="btn {activeTab === key ? 'btn-primary' : 'btn-secondary'}" on:click={() => (activeTab = key as typeof activeTab)}>{label}</button>
		{/each}
	</div>

	{#if loading}
		<div class="flex items-center justify-center py-16"><div class="spinner"></div></div>
	{:else if activeTab === 'overview'}
		<div class="card mb-4">
			<div class="card-body flex gap-3 items-end flex-wrap" style="padding:1rem">
				<div class="form-group"><label class="form-label" for="f-from">From</label><input id="f-from" type="date" class="input" bind:value={from} on:change={loadSummary} /></div>
				<div class="form-group"><label class="form-label" for="f-to">To</label><input id="f-to" type="date" class="input" bind:value={to} on:change={loadSummary} /></div>
			</div>
		</div>
		<div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-4 mb-6">
			{#each [
				['Sales (incl. GST)', summary.total_sales, 'text-slate-900'],
				['Net sales (taxable)', summary.net_sales, 'text-slate-900'],
				['Gross profit (est.)', summary.gross_profit, toNumber(summary.gross_profit) >= 0 ? 'text-green-600' : 'text-red-600'],
				['Net profit (est.)', summary.net_profit, toNumber(summary.net_profit) >= 0 ? 'text-green-600' : 'text-red-600'],
				['Purchases', summary.total_purchases, 'text-slate-900'],
				['Expenses (approved)', summary.total_expenses, 'text-amber-600'],
				['Collected', summary.collected, 'text-green-600'],
				['Paid out', summary.paid_out, 'text-red-600']
			] as [label, value, color]}
				<div class="card"><div class="card-body">
					<p class="text-xs font-semibold uppercase tracking-widest text-slate-400 mb-1">{label}</p>
					<p class="text-2xl font-bold {color}">{formatINR(value)}</p>
				</div></div>
			{/each}
		</div>
		<div class="grid grid-cols-1 md:grid-cols-3 gap-4">
			<div class="card"><div class="card-body">
				<p class="text-xs font-semibold uppercase tracking-widest text-green-600 mb-1">Receivables (all time)</p>
				<p class="text-2xl font-bold">{formatINR(summary.receivables)}</p>
			</div></div>
			<div class="card"><div class="card-body">
				<p class="text-xs font-semibold uppercase tracking-widest text-red-600 mb-1">Payables (all time)</p>
				<p class="text-2xl font-bold">{formatINR(summary.payables)}</p>
			</div></div>
			<div class="card"><div class="card-body">
				<p class="text-xs font-semibold uppercase tracking-widest text-amber-600 mb-1">Pending expense approvals</p>
				<p class="text-2xl font-bold">{formatINR(summary.pending_expenses)}</p>
			</div></div>
		</div>
		<p class="text-xs text-slate-400 mt-4">Profit estimates use each product's current cost price as cost of goods sold.</p>
	{:else if activeTab === 'receivables'}
		<div class="card"><div class="table-container">
			<table class="table">
				<thead><tr><th>Invoice</th><th>Customer</th><th class="text-right">Total</th><th class="text-right">Outstanding</th><th>Due</th><th>Status</th><th></th></tr></thead>
				<tbody>
				{#each receivables as r}
					<tr>
						<td class="font-mono text-sm">{r.invoice_number}</td>
						<td>{r.customer_name ?? 'Walk-in'}{#if r.customer_phone}<div class="text-xs text-slate-400">{r.customer_phone}</div>{/if}</td>
						<td class="text-right">{formatINR(r.grand_total)}</td>
						<td class="text-right font-semibold text-green-700">{formatINR(r.outstanding_amount)}</td>
						<td class="text-sm {r.is_overdue ? 'text-red-600 font-semibold' : 'text-slate-600'}">{formatDate(r.due_date)}</td>
						<td><span class="badge {r.is_overdue ? 'badge-danger' : STATUS_BADGE[r.status] ?? 'badge-warning'}">{r.is_overdue ? 'overdue' : humanize(r.status)}</span></td>
						<td class="text-right">
							{#if canApprove}<button class="btn btn-ghost btn-sm" on:click={() => openDueDate('invoice', r)}>Change due date</button>{/if}
						</td>
					</tr>
				{:else}
					<tr><td colspan="7" class="text-center text-slate-400 py-8">No receivables outstanding</td></tr>
				{/each}
				</tbody>
			</table>
		</div></div>
	{:else if activeTab === 'payables'}
		<div class="card"><div class="table-container">
			<table class="table">
				<thead><tr><th>PO</th><th>Supplier</th><th class="text-right">Total</th><th class="text-right">Outstanding</th><th>Due</th><th>Status</th><th></th></tr></thead>
				<tbody>
				{#each payables as p}
					<tr class="clickable" on:click={() => (location.href = `/purchases/${p.id}`)}>
						<td class="font-mono text-sm">{p.po_number}</td>
						<td class="font-medium">{p.supplier_name}</td>
						<td class="text-right">{formatINR(p.grand_total)}</td>
						<td class="text-right font-semibold text-red-600">{formatINR(p.outstanding_amount)}</td>
						<td class="text-sm {p.is_overdue ? 'text-red-600 font-semibold' : 'text-slate-600'}">{formatDate(p.due_date)}</td>
						<td><span class="badge {p.is_overdue ? 'badge-danger' : STATUS_BADGE[p.payment_status] ?? 'badge-warning'}">{p.is_overdue ? 'overdue' : humanize(p.payment_status)}</span></td>
						<td class="text-right">
							{#if canApprove}<button class="btn btn-ghost btn-sm" on:click|stopPropagation={() => openDueDate('purchase', p)}>Change due date</button>{/if}
						</td>
					</tr>
				{:else}
					<tr><td colspan="7" class="text-center text-slate-400 py-8">No payables outstanding</td></tr>
				{/each}
				</tbody>
			</table>
		</div></div>
	{:else}
		<div class="card"><div class="table-container">
			<table class="table">
				<thead><tr><th>Date</th><th>Category</th><th>Description</th><th>Method</th><th class="text-right">Amount</th><th>Status</th><th></th></tr></thead>
				<tbody>
				{#each expenses as e}
					<tr>
						<td class="text-sm">{formatDate(e.date)}</td>
						<td class="font-medium">{e.category}</td>
						<td class="text-slate-500 text-sm">{e.description ?? '—'}{#if e.created_by_name}<div class="text-xs text-slate-400">by {e.created_by_name}</div>{/if}</td>
						<td class="text-sm">{humanize(e.payment_method)}</td>
						<td class="text-right font-semibold">{formatINR(e.amount)}</td>
						<td><span class="badge {STATUS_BADGE[e.approval_status] ?? 'badge-secondary'}">{e.approval_status}</span></td>
						<td class="text-right whitespace-nowrap">
							{#if canApprove && e.approval_status === 'pending'}
								<button class="btn btn-ghost btn-sm text-green-700" on:click={() => review(e.id, 'approve')}>Approve</button>
								<button class="btn btn-ghost btn-sm text-red-600" on:click={() => review(e.id, 'reject')}>Reject</button>
							{/if}
							{#if canEditExpense(e)}
								<button class="btn btn-ghost btn-sm" on:click={() => openEditExpense(e)}>Edit</button>
								<button class="btn btn-ghost btn-sm text-red-600" on:click={() => removeExpense(e)}>Delete</button>
							{/if}
						</td>
					</tr>
				{:else}
					<tr><td colspan="7" class="text-center text-slate-400 py-8">No expenses recorded</td></tr>
				{/each}
				</tbody>
			</table>
		</div></div>
	{/if}
</div>

{#if showExpense}
	<div class="modal-overlay" on:click|self={closeExpense} on:keydown={(e) => e.key === 'Escape' && closeExpense()}
		role="dialog" aria-modal="true" aria-label={editExpenseId ? 'Edit expense' : 'Record expense'} tabindex="-1">
		<form class="modal" on:submit|preventDefault={saveExpense}>
			<div class="modal-header">
				<h2 class="modal-title">{editExpenseId ? 'Edit expense' : 'Record expense'}</h2>
				<button type="button" class="modal-close" on:click={closeExpense} aria-label="Close">×</button>
			</div>
			<div class="modal-body grid grid-cols-2 gap-4">
				<div class="form-group col-span-2"><label class="form-label required" for="ex-cat">Category</label>
					<input id="ex-cat" class="form-control" list="ex-cats" bind:value={expense.category} required />
					<datalist id="ex-cats">{#each EXPENSE_CATEGORIES as c}<option value={c}></option>{/each}</datalist></div>
				<div class="form-group"><label class="form-label required" for="ex-amt">Amount (₹)</label>
					<input id="ex-amt" class="form-control" type="number" step="0.01" min="0.01" bind:value={expense.amount} required /></div>
				<div class="form-group"><label class="form-label" for="ex-date">Date</label>
					<input id="ex-date" class="form-control" type="date" max={isoDate()} bind:value={expense.date} /></div>
				<div class="form-group"><label class="form-label" for="ex-m">Paid via</label>
					<select id="ex-m" class="form-control" bind:value={expense.payment_method}>
						<option value="cash">Cash</option><option value="upi">UPI</option>
						<option value="bank_transfer">Bank transfer</option><option value="cheque">Cheque</option>
					</select></div>
				<div class="form-group"><label class="form-label" for="ex-ref">Reference</label>
					<input id="ex-ref" class="form-control" bind:value={expense.reference} /></div>
				<div class="form-group col-span-2"><label class="form-label" for="ex-desc">Description</label>
					<textarea id="ex-desc" class="form-control" rows="2" bind:value={expense.description}></textarea></div>
			</div>
			<div class="modal-footer">
				<button type="button" class="btn btn-secondary" on:click={closeExpense}>Cancel</button>
				<button type="submit" class="btn btn-primary" disabled={saving}>{saving ? 'Saving…' : editExpenseId ? 'Update expense' : 'Save expense'}</button>
			</div>
		</form>
	</div>
{/if}

{#if dueEdit}
	<div class="modal-overlay" on:click|self={() => (dueEdit = null)} on:keydown={(e) => e.key === 'Escape' && (dueEdit = null)}
		role="dialog" aria-modal="true" aria-label="Change due date" tabindex="-1">
		<form class="modal" on:submit|preventDefault={saveDueDate}>
			<div class="modal-header">
				<h2 class="modal-title">Change due date · {dueEdit.label}</h2>
				<button type="button" class="modal-close" on:click={() => (dueEdit = null)} aria-label="Close">×</button>
			</div>
			<div class="modal-body">
				<div class="form-group"><label class="form-label required" for="due-date">New due date</label>
					<input id="due-date" class="form-control" type="date" min={dueEdit.min} bind:value={dueEdit.due_date} required />
					<span class="form-hint">Payment reminders will follow the new date.</span></div>
			</div>
			<div class="modal-footer">
				<button type="button" class="btn btn-secondary" on:click={() => (dueEdit = null)}>Cancel</button>
				<button type="submit" class="btn btn-primary" disabled={saving}>{saving ? 'Saving…' : 'Update due date'}</button>
			</div>
		</form>
	</div>
{/if}
