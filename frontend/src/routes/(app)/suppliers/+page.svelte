<script lang="ts">
	import { onMount } from 'svelte';
	import { supplierApi } from '$lib/api/client';
	import { toasts } from '$stores/toasts';

	let suppliers: Record<string, unknown>[] = [];
	let total = 0;
	let loading = true;
	let search = '';
	let showModal = false;
	let editId: string | null = null;
	let submitting = false;

	const empty = () => ({
		business_name: '', contact_person: '', phone: '', email: '',
		address: '', gstin: '', payment_terms_days: '30', credit_limit: '0', notes: ''
	});
	let form = empty();

	const formatINR = (v: unknown) =>
		new Intl.NumberFormat('en-IN', { style: 'currency', currency: 'INR', maximumFractionDigits: 0 }).format(Number(v ?? 0));

	async function load() {
		loading = true;
		try {
			const res: any = await supplierApi.list({ search, limit: 50 });
			suppliers = res.data ?? res ?? [];
			total = res.total ?? suppliers.length;
		} catch (e: any) { toasts.error('Failed to load suppliers'); }
		loading = false;
	}

	async function submit() {
		submitting = true;
		try {
			const data = { ...form, payment_terms_days: Number(form.payment_terms_days), credit_limit: Number(form.credit_limit) };
			if (editId) await supplierApi.update(editId, data);
			else await supplierApi.create(data);
			toasts.success(editId ? 'Supplier updated' : 'Supplier added');
			showModal = false; editId = null; form = empty();
			await load();
		} catch (e: any) { toasts.error(e.message ?? 'Failed to save'); }
		submitting = false;
	}

	function openEdit(s: Record<string, unknown>) {
		editId = String(s.id);
		form = {
			business_name: String(s.business_name ?? ''), contact_person: String(s.contact_person ?? ''),
			phone: String(s.phone ?? ''), email: String(s.email ?? ''), address: String(s.address ?? ''),
			gstin: String(s.gstin ?? ''), payment_terms_days: String(s.payment_terms_days ?? 30),
			credit_limit: String(s.credit_limit ?? 0), notes: String(s.notes ?? '')
		};
		showModal = true;
	}

	onMount(load);
	$: if (search !== undefined) { load(); }
</script>

<svelte:head><title>Suppliers — RARA Enterprise ERP</title></svelte:head>

<div class="page-container">
	<div class="page-header">
		<div>
			<h1 class="page-title">Suppliers</h1>
			<p class="page-subtitle">{total} supplier{total !== 1 ? 's' : ''} registered</p>
		</div>
		<button class="btn btn-primary" on:click={() => { form = empty(); editId = null; showModal = true; }}>
			+ Add Supplier
		</button>
	</div>

	<div class="card mb-4">
		<div class="card-body" style="padding: 1rem;">
			<input class="input" placeholder="Search by name, phone, GSTIN..." bind:value={search} style="max-width:400px;" />
		</div>
	</div>

	{#if loading}
		<div class="flex items-center justify-center py-16 text-slate-400">Loading...</div>
	{:else if suppliers.length === 0}
		<div class="card"><div class="card-body text-center py-16">
			<p class="text-slate-400 text-lg mb-2">No suppliers found</p>
			<p class="text-slate-500 text-sm">Add your first supplier to get started</p>
		</div></div>
	{:else}
		<div class="card">
			<div class="table-container">
				<table class="table">
					<thead><tr>
						<th>Business</th><th>Contact</th><th>Phone</th>
						<th>GSTIN</th><th>Outstanding</th><th>Credit Limit</th><th>Actions</th>
					</tr></thead>
					<tbody>
					{#each suppliers as s}
						<tr>
							<td>
								<div class="font-semibold text-slate-800">{s.business_name}</div>
								{#if s.contact_person}<div class="text-xs text-slate-500">{s.contact_person}</div>{/if}
							</td>
							<td class="text-slate-600">{s.email ?? '—'}</td>
							<td class="font-mono text-sm">{s.phone}</td>
							<td>
								{#if s.gstin}
									<span class="badge badge-{s.gst_verified ? 'success' : 'warning'}">{s.gstin}</span>
								{:else}<span class="text-slate-400">—</span>{/if}
							</td>
							<td class="font-semibold text-red-600">{formatINR(s.outstanding_balance)}</td>
							<td>{formatINR(s.credit_limit)}</td>
							<td>
								<button class="btn btn-ghost btn-sm" on:click={() => openEdit(s)}>Edit</button>
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
<div class="modal-overlay" role="presentation" on:click|self={() => { showModal = false; }}>
	<div class="modal" style="max-width:600px;">
		<div class="modal-header">
			<h2 class="modal-title">{editId ? 'Edit Supplier' : 'Add Supplier'}</h2>
			<button class="modal-close" on:click={() => { showModal = false; }}>✕</button>
		</div>
		<form on:submit|preventDefault={submit}>
			<div class="modal-body">
				<div class="grid grid-cols-2 gap-4">
					<div class="form-group col-span-2">
						<label class="form-label" for="suppliers-f1">Business Name *</label>
						<input id="suppliers-f1" class="input" bind:value={form.business_name} required />
					</div>
					<div class="form-group">
						<label class="form-label" for="suppliers-f2">Contact Person</label>
						<input id="suppliers-f2" class="input" bind:value={form.contact_person} />
					</div>
					<div class="form-group">
						<label class="form-label" for="suppliers-f3">Phone *</label>
						<input id="suppliers-f3" class="input" bind:value={form.phone} required />
					</div>
					<div class="form-group">
						<label class="form-label" for="suppliers-f4">Email</label>
						<input id="suppliers-f4" class="input" type="email" bind:value={form.email} />
					</div>
					<div class="form-group">
						<label class="form-label" for="suppliers-f5">GSTIN</label>
						<input id="suppliers-f5" class="input" bind:value={form.gstin} maxlength="15" />
					</div>
					<div class="form-group">
						<label class="form-label" for="suppliers-f6">Payment Terms (days)</label>
						<input id="suppliers-f6" class="input" type="number" bind:value={form.payment_terms_days} min="0" />
					</div>
					<div class="form-group">
						<label class="form-label" for="suppliers-f7">Credit Limit (₹)</label>
						<input id="suppliers-f7" class="input" type="number" bind:value={form.credit_limit} min="0" />
					</div>
					<div class="form-group col-span-2">
						<label class="form-label" for="suppliers-f8">Address</label>
						<textarea id="suppliers-f8" class="input" rows="2" bind:value={form.address}></textarea>
					</div>
					<div class="form-group col-span-2">
						<label class="form-label" for="suppliers-f9">Notes</label>
						<textarea id="suppliers-f9" class="input" rows="2" bind:value={form.notes}></textarea>
					</div>
				</div>
			</div>
			<div class="modal-footer">
				<button type="button" class="btn btn-secondary" on:click={() => { showModal = false; }}>Cancel</button>
				<button type="submit" class="btn btn-primary" disabled={submitting}>
					{submitting ? 'Saving...' : editId ? 'Update' : 'Add Supplier'}
				</button>
			</div>
		</form>
	</div>
</div>
{/if}
