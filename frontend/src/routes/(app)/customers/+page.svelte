<script lang="ts">
	import { onMount } from 'svelte';
	import { customerApi } from '$lib/api/client';
	import { toasts } from '$stores/toasts';

	let customers: Record<string, unknown>[] = [];
	let total = 0;
	let loading = true;
	let search = '';
	let showModal = false;
	let editId: string | null = null;
	let submitting = false;

	const empty = () => ({
		name: '', business_name: '', phone: '', email: '',
		billing_address: '', gstin: '', customer_type: 'retail',
		credit_limit: '0', credit_period_days: '0', notes: ''
	});
	let form = empty();

	const formatINR = (v: unknown) =>
		new Intl.NumberFormat('en-IN', { style: 'currency', currency: 'INR', maximumFractionDigits: 0 }).format(Number(v ?? 0));

	async function load() {
		loading = true;
		try {
			const res: any = await customerApi.list({ search, limit: 50 });
			customers = res.data ?? res ?? [];
			total = res.total ?? customers.length;
		} catch { toasts.error('Failed to load customers'); }
		loading = false;
	}

	async function submit() {
		submitting = true;
		try {
			const data = { ...form, credit_limit: Number(form.credit_limit), credit_period_days: Number(form.credit_period_days) };
			if (editId) await customerApi.update(editId, data);
			else await customerApi.create(data);
			toasts.success(editId ? 'Customer updated' : 'Customer added');
			showModal = false; editId = null; form = empty();
			await load();
		} catch (e: any) { toasts.error(e.message ?? 'Failed to save'); }
		submitting = false;
	}

	function openEdit(c: Record<string, unknown>) {
		editId = String(c.id);
		form = {
			name: String(c.name ?? ''), business_name: String(c.business_name ?? ''),
			phone: String(c.phone ?? ''), email: String(c.email ?? ''),
			billing_address: String(c.billing_address ?? ''), gstin: String(c.gstin ?? ''),
			customer_type: String(c.customer_type ?? 'retail'),
			credit_limit: String(c.credit_limit ?? 0), credit_period_days: String(c.credit_period_days ?? 0),
			notes: String(c.notes ?? '')
		};
		showModal = true;
	}

	onMount(load);
	$: if (search !== undefined) { load(); }
</script>

<svelte:head><title>Customers — RARA Enterprise ERP</title></svelte:head>

<div class="page-container">
	<div class="page-header">
		<div>
			<h1 class="page-title">Customers</h1>
			<p class="page-subtitle">{total} customer{total !== 1 ? 's' : ''} registered</p>
		</div>
		<button class="btn btn-primary" on:click={() => { form = empty(); editId = null; showModal = true; }}>
			+ Add Customer
		</button>
	</div>

	<div class="card mb-4">
		<div class="card-body" style="padding:1rem;">
			<input class="input" placeholder="Search by name, phone, GSTIN..." bind:value={search} style="max-width:400px;" />
		</div>
	</div>

	{#if loading}
		<div class="flex items-center justify-center py-16 text-slate-400">Loading...</div>
	{:else if customers.length === 0}
		<div class="card"><div class="card-body text-center py-16">
			<p class="text-slate-400 text-lg mb-2">No customers found</p>
			<p class="text-slate-500 text-sm">Add your first customer to get started</p>
		</div></div>
	{:else}
		<div class="card">
			<div class="table-container">
				<table class="table">
					<thead><tr>
						<th>Customer</th><th>Phone</th><th>Type</th>
						<th>GSTIN</th><th>Outstanding</th><th>Credit Limit</th><th>Actions</th>
					</tr></thead>
					<tbody>
					{#each customers as c}
						<tr>
							<td>
								<div class="font-semibold text-slate-800">{c.name}</div>
								{#if c.business_name}<div class="text-xs text-slate-500">{c.business_name}</div>{/if}
							</td>
							<td class="font-mono text-sm">{c.phone}</td>
							<td><span class="badge badge-{c.customer_type === 'wholesale' ? 'primary' : 'info'}">{c.customer_type}</span></td>
							<td>
								{#if c.gstin}<span class="badge badge-{c.gst_verified ? 'success' : 'warning'}">{c.gstin}</span>
								{:else}<span class="text-slate-400">—</span>{/if}
							</td>
							<td class="font-semibold text-red-600">{formatINR(c.outstanding_balance)}</td>
							<td>{formatINR(c.credit_limit)}</td>
							<td>
								<button class="btn btn-ghost btn-sm" on:click={() => openEdit(c)}>Edit</button>
							</td>
						</tr>
					{/each}
					</tbody>
				</table>
			</div>
		</div>
	{/if}
</div>

{#if showModal}
<div class="modal-overlay" on:click|self={() => { showModal = false; }}>
	<div class="modal" style="max-width:600px;">
		<div class="modal-header">
			<h2 class="modal-title">{editId ? 'Edit Customer' : 'Add Customer'}</h2>
			<button class="modal-close" on:click={() => { showModal = false; }}>✕</button>
		</div>
		<form on:submit|preventDefault={submit}>
			<div class="modal-body">
				<div class="grid grid-cols-2 gap-4">
					<div class="form-group"><label class="form-label">Full Name *</label><input class="input" bind:value={form.name} required /></div>
					<div class="form-group"><label class="form-label">Business Name</label><input class="input" bind:value={form.business_name} /></div>
					<div class="form-group"><label class="form-label">Phone *</label><input class="input" bind:value={form.phone} required /></div>
					<div class="form-group"><label class="form-label">Email</label><input class="input" type="email" bind:value={form.email} /></div>
					<div class="form-group"><label class="form-label">Customer Type</label>
						<select class="input" bind:value={form.customer_type}>
							<option value="retail">Retail</option><option value="wholesale">Wholesale</option>
						</select>
					</div>
					<div class="form-group"><label class="form-label">GSTIN</label><input class="input" bind:value={form.gstin} maxlength="15" /></div>
					<div class="form-group"><label class="form-label">Credit Limit (₹)</label><input class="input" type="number" bind:value={form.credit_limit} min="0" /></div>
					<div class="form-group"><label class="form-label">Credit Period (days)</label><input class="input" type="number" bind:value={form.credit_period_days} min="0" /></div>
					<div class="form-group col-span-2"><label class="form-label">Billing Address</label><textarea class="input" rows="2" bind:value={form.billing_address}></textarea></div>
					<div class="form-group col-span-2"><label class="form-label">Notes</label><textarea class="input" rows="2" bind:value={form.notes}></textarea></div>
				</div>
			</div>
			<div class="modal-footer">
				<button type="button" class="btn btn-secondary" on:click={() => { showModal = false; }}>Cancel</button>
				<button type="submit" class="btn btn-primary" disabled={submitting}>{submitting ? 'Saving...' : editId ? 'Update' : 'Add Customer'}</button>
			</div>
		</form>
	</div>
</div>
{/if}
