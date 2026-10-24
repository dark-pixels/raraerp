<script lang="ts">
	import { onMount } from 'svelte';
	import { usersApi } from '$lib/api/client';
	import { toasts } from '$stores/toasts';

	let users: Record<string, unknown>[] = [];
	let loading = true;
	let showModal = false;
	let submitting = false;
	let editId: string | null = null;
	const empty = () => ({ full_name: '', email: '', role: 'staff', password: '' });
	let form = empty();

	async function load() {
		loading = true;
		try {
			const res: any = await usersApi.list({ limit: 50 });
			users = res.data ?? res ?? [];
		} catch { toasts.error('Failed to load staff'); }
		loading = false;
	}

	async function submit() {
		submitting = true;
		try {
			if (editId) {
				const { password, ...rest } = form;
				await usersApi.update(editId, rest);
			} else {
				await usersApi.create(form);
			}
			toasts.success(editId ? 'User updated' : 'Staff member added');
			showModal = false; editId = null; form = empty();
			await load();
		} catch (e: any) { toasts.error(e.message ?? 'Failed'); }
		submitting = false;
	}

	async function deactivate(id: string) {
		if (!confirm('Deactivate this user?')) return;
		try { await usersApi.deactivate(id); toasts.success('User deactivated'); await load(); }
		catch (e: any) { toasts.error(e.message ?? 'Failed'); }
	}

	const ROLE_COLORS: Record<string, string> = { super_admin: 'badge-danger', admin: 'badge-primary', staff: 'badge-info' };
	onMount(load);
</script>

<svelte:head><title>Staff Management — RARA Enterprise ERP</title></svelte:head>

<div class="page-container">
	<div class="page-header">
		<div>
			<h1 class="page-title">Staff Management</h1>
			<p class="page-subtitle">{users.length} user accounts</p>
		</div>
		<button class="btn btn-primary" on:click={() => { form = empty(); editId = null; showModal = true; }}>+ Add Staff</button>
	</div>

	{#if loading}
		<div class="flex items-center justify-center py-16 text-slate-400">Loading...</div>
	{:else}
	<div class="card">
		<div class="table-container">
			<table class="table">
				<thead><tr><th>Name</th><th>Email</th><th>Role</th><th>Status</th><th>Actions</th></tr></thead>
				<tbody>
				{#each users as u}
					<tr>
						<td class="font-semibold">{u.full_name}</td>
						<td class="text-slate-600">{u.email}</td>
						<td><span class="badge {ROLE_COLORS[String(u.role)] ?? 'badge-secondary'}">{String(u.role).replace('_',' ')}</span></td>
						<td><span class="badge {u.is_active ? 'badge-success' : 'badge-secondary'}">{u.is_active ? 'Active' : 'Inactive'}</span></td>
						<td class="flex gap-1">
							<button class="btn btn-ghost btn-sm" on:click={() => { editId = String(u.id); form = { full_name: String(u.full_name), email: String(u.email), role: String(u.role), password: '' }; showModal = true; }}>Edit</button>
							{#if u.is_active}<button class="btn btn-ghost btn-sm text-red-500" on:click={() => deactivate(String(u.id))}>Deactivate</button>{/if}
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
<div class="modal-overlay" on:click|self={() => showModal = false}>
	<div class="modal">
		<div class="modal-header">
			<h2 class="modal-title">{editId ? 'Edit User' : 'Add Staff Member'}</h2>
			<button class="modal-close" on:click={() => showModal = false}>✕</button>
		</div>
		<form on:submit|preventDefault={submit}>
			<div class="modal-body">
				<div class="form-group"><label class="form-label">Full Name *</label><input class="input" bind:value={form.full_name} required /></div>
				<div class="form-group"><label class="form-label">Email *</label><input class="input" type="email" bind:value={form.email} required /></div>
				<div class="form-group"><label class="form-label">Role</label>
					<select class="input" bind:value={form.role}>
						<option value="staff">Staff</option>
						<option value="admin">Admin</option>
						<option value="super_admin">Super Admin</option>
					</select>
				</div>
				{#if !editId}
					<div class="form-group"><label class="form-label">Password *</label><input class="input" type="password" bind:value={form.password} required /></div>
				{/if}
			</div>
			<div class="modal-footer">
				<button type="button" class="btn btn-secondary" on:click={() => showModal = false}>Cancel</button>
				<button type="submit" class="btn btn-primary" disabled={submitting}>{submitting ? 'Saving...' : editId ? 'Update' : 'Add'}</button>
			</div>
		</form>
	</div>
</div>
{/if}
