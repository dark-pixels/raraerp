<script lang="ts">
	import { onMount } from 'svelte';
	import { usersApi, errorMessage } from '$api/client';
	import { currentUser, hasPermission } from '$stores/auth';
	import { toasts } from '$stores/toasts';
	import { formatDateTime, humanize } from '$utils/format';

	type User = { id: string; full_name: string; email: string; role: string; is_active: boolean; last_login?: string | null };
	type Perm = { key: string; label: string; description?: string };

	let users: User[] = [];
	let loading = true;
	let search = '';

	let modal: '' | 'edit' | 'password' | 'permissions' = '';
	let submitting = false;
	let editId: string | null = null;
	const empty = () => ({ full_name: '', email: '', role: 'staff', password: '' });
	let form = empty();
	let newPassword = '';

	let allPerms: Perm[] = [];
	let selectedPerms = new Set<string>();
	let roleDefaults = new Set<string>();

	$: me = $currentUser;
	$: isSuper = me?.role === 'super_admin';
	$: canCreate = hasPermission(me, 'users.create');
	$: canUpdate = hasPermission(me, 'users.update');
	$: canDeactivate = hasPermission(me, 'users.deactivate');
	$: canManageRoles = hasPermission(me, 'roles.manage');
	$: assignableRoles = isSuper ? ['staff', 'admin', 'super_admin'] : ['staff'];
	$: filtered = users.filter((u) => !search || `${u.full_name} ${u.email}`.toLowerCase().includes(search.toLowerCase()));

	$: canManage = (u: User) => isSuper || (u.role === 'staff' && me?.role === 'admin');

	async function load() {
		loading = true;
		try {
			const res: any = await usersApi.list({ per_page: 200 });
			users = res.data ?? [];
		} catch (e) { toasts.error('Failed to load staff', errorMessage(e)); }
		loading = false;
	}

	function openCreate() { form = empty(); editId = null; modal = 'edit'; }
	function openEdit(u: User) { editId = u.id; form = { full_name: u.full_name, email: u.email, role: u.role, password: '' }; modal = 'edit'; }
	function openPassword(u: User) { editId = u.id; newPassword = ''; modal = 'password'; }
	function close() { modal = ''; editId = null; }

	async function submit() {
		submitting = true;
		try {
			if (editId) {
				const { password: _ignored, ...rest } = form;
				await usersApi.update(editId, rest);
				toasts.success('User updated');
			} else {
				await usersApi.create(form);
				toasts.success('Staff member added');
			}
			close();
			await load();
		} catch (e) { toasts.error('Could not save user', errorMessage(e)); }
		submitting = false;
	}

	async function resetPassword() {
		if (!editId) return;
		submitting = true;
		try {
			const res: any = await usersApi.resetPassword(editId, newPassword);
			toasts.success('Password reset', res.message);
			close();
		} catch (e) { toasts.error('Could not reset password', errorMessage(e)); }
		submitting = false;
	}

	async function openPermissions(u: User) {
		editId = u.id;
		modal = 'permissions';
		try {
			const [all, mine]: any[] = await Promise.all([usersApi.allPermissions(), usersApi.permissions(u.id)]);
			allPerms = all.data ?? [];
			selectedPerms = new Set(mine.permissions ?? []);
			roleDefaults = new Set(mine.role_defaults ?? []);
		} catch (e) { toasts.error('Failed to load permissions', errorMessage(e)); close(); }
	}

	function togglePerm(key: string) {
		selectedPerms.has(key) ? selectedPerms.delete(key) : selectedPerms.add(key);
		selectedPerms = selectedPerms;
	}

	async function savePermissions() {
		if (!editId) return;
		submitting = true;
		try {
			await usersApi.updatePermissions(editId, [...selectedPerms]);
			toasts.success('Permissions updated');
			close();
		} catch (e) { toasts.error('Could not update permissions', errorMessage(e)); }
		submitting = false;
	}

	async function toggleActive(u: User) {
		if (u.is_active && !confirm(`Deactivate ${u.full_name}? They will be signed out immediately.`)) return;
		try {
			if (u.is_active) await usersApi.deactivate(u.id);
			else await usersApi.update(u.id, { is_active: true });
			toasts.success(u.is_active ? 'User deactivated' : 'User reactivated');
			await load();
		} catch (e) { toasts.error('Action failed', errorMessage(e)); }
	}

	const ROLE_COLORS: Record<string, string> = { super_admin: 'badge-danger', admin: 'badge-primary', staff: 'badge-info' };
	onMount(load);
</script>

<svelte:head><title>Staff Management — RARA Enterprise ERP</title></svelte:head>

<div class="page-container">
	<div class="page-header flex items-center justify-between gap-4 flex-wrap">
		<div>
			<h1 class="page-title">Staff Management</h1>
			<p class="page-subtitle">{users.length} user accounts</p>
		</div>
		<div class="flex gap-2">
			<input class="input" style="max-width:240px" placeholder="Search staff…" bind:value={search} aria-label="Search staff" />
			{#if canCreate}<button class="btn btn-primary" on:click={openCreate}>+ Add Staff</button>{/if}
		</div>
	</div>

	{#if loading}
		<div class="flex items-center justify-center py-16"><div class="spinner"></div></div>
	{:else}
		<div class="card">
			<div class="table-container">
				<table class="table">
					<thead><tr><th>Name</th><th>Email</th><th>Role</th><th>Status</th><th>Last sign-in</th><th class="text-right">Actions</th></tr></thead>
					<tbody>
					{#each filtered as u (u.id)}
						<tr class={u.is_active ? '' : 'opacity-60'}>
							<td class="font-semibold">{u.full_name}{#if u.id === me?.id} <span class="text-xs text-slate-400">(you)</span>{/if}</td>
							<td class="text-slate-600">{u.email}</td>
							<td><span class="badge {ROLE_COLORS[u.role] ?? 'badge-secondary'}">{humanize(u.role)}</span></td>
							<td><span class="badge {u.is_active ? 'badge-success' : 'badge-secondary'}">{u.is_active ? 'Active' : 'Inactive'}</span></td>
							<td class="text-sm text-slate-500">{u.last_login ? formatDateTime(u.last_login) : 'Never'}</td>
							<td class="text-right whitespace-nowrap">
								{#if u.id !== me?.id && canManage(u)}
									{#if canUpdate}<button class="btn btn-ghost btn-sm" on:click={() => openEdit(u)}>Edit</button>{/if}
									{#if canUpdate}<button class="btn btn-ghost btn-sm" on:click={() => openPassword(u)}>Reset password</button>{/if}
									{#if canManageRoles && u.role !== 'super_admin'}<button class="btn btn-ghost btn-sm" on:click={() => openPermissions(u)}>Permissions</button>{/if}
									{#if canDeactivate || canUpdate}
										<button class="btn btn-ghost btn-sm {u.is_active ? 'text-red-500' : 'text-green-700'}" on:click={() => toggleActive(u)}>{u.is_active ? 'Deactivate' : 'Reactivate'}</button>
									{/if}
								{/if}
							</td>
						</tr>
					{:else}
						<tr><td colspan="6" class="text-center text-slate-400 py-8">No users found</td></tr>
					{/each}
					</tbody>
				</table>
			</div>
		</div>
	{/if}
</div>

{#if modal}
	<div class="modal-overlay" on:click|self={close} on:keydown={(e) => e.key === 'Escape' && close()} role="dialog" aria-modal="true" tabindex="-1" aria-label="Staff dialog">
		{#if modal === 'edit'}
			<form class="modal" on:submit|preventDefault={submit}>
				<div class="modal-header">
					<h2 class="modal-title">{editId ? 'Edit user' : 'Add staff member'}</h2>
					<button type="button" class="modal-close" on:click={close} aria-label="Close">×</button>
				</div>
				<div class="modal-body space-y-4">
					<div class="form-group"><label class="form-label required" for="u-name">Full name</label><input id="u-name" class="input" bind:value={form.full_name} required /></div>
					<div class="form-group"><label class="form-label required" for="u-email">Email</label><input id="u-email" class="input" type="email" autocomplete="off" bind:value={form.email} required /></div>
					<div class="form-group"><label class="form-label" for="u-role">Role</label>
						<select id="u-role" class="input" bind:value={form.role}>
							{#each assignableRoles as r}<option value={r}>{humanize(r)}</option>{/each}
						</select>
						<span class="form-hint">Staff: billing and stock. Admin: purchases, finance and staff. Super admin: everything.</span>
					</div>
					{#if !editId}
						<div class="form-group"><label class="form-label required" for="u-pass">Password</label>
							<input id="u-pass" class="input" type="password" autocomplete="new-password" minlength="8" bind:value={form.password} required />
							<span class="form-hint">At least 8 characters with letters and numbers.</span></div>
					{/if}
				</div>
				<div class="modal-footer">
					<button type="button" class="btn btn-secondary" on:click={close}>Cancel</button>
					<button type="submit" class="btn btn-primary" disabled={submitting}>{submitting ? 'Saving…' : editId ? 'Update' : 'Add'}</button>
				</div>
			</form>
		{:else if modal === 'password'}
			<form class="modal" on:submit|preventDefault={resetPassword}>
				<div class="modal-header">
					<h2 class="modal-title">Reset password</h2>
					<button type="button" class="modal-close" on:click={close} aria-label="Close">×</button>
				</div>
				<div class="modal-body space-y-3">
					<p class="text-sm text-slate-500">The user will be signed out of all devices and must use the new password.</p>
					<div class="form-group"><label class="form-label required" for="u-newpass">New password</label>
						<input id="u-newpass" class="input" type="password" autocomplete="new-password" minlength="8" bind:value={newPassword} required /></div>
				</div>
				<div class="modal-footer">
					<button type="button" class="btn btn-secondary" on:click={close}>Cancel</button>
					<button type="submit" class="btn btn-primary" disabled={submitting}>{submitting ? 'Saving…' : 'Reset password'}</button>
				</div>
			</form>
		{:else}
			<form class="modal modal-lg" on:submit|preventDefault={savePermissions}>
				<div class="modal-header">
					<h2 class="modal-title">Permissions</h2>
					<button type="button" class="modal-close" on:click={close} aria-label="Close">×</button>
				</div>
				<div class="modal-body">
					<p class="text-sm text-slate-500 mb-4">Ticked items are what this user can do. Items marked "role default" come from their role.</p>
					<div class="grid grid-cols-1 md:grid-cols-2 gap-2">
						{#each allPerms as p (p.key)}
							<label class="flex items-start gap-2 p-2 rounded-lg hover:bg-slate-50 cursor-pointer">
								<input type="checkbox" checked={selectedPerms.has(p.key)} on:change={() => togglePerm(p.key)} class="mt-1" />
								<span>
									<span class="font-medium text-sm">{p.label}</span>
									{#if roleDefaults.has(p.key)}<span class="text-xs text-slate-400"> · role default</span>{/if}
									{#if p.description}<span class="block text-xs text-slate-500">{p.description}</span>{/if}
								</span>
							</label>
						{/each}
					</div>
				</div>
				<div class="modal-footer">
					<button type="button" class="btn btn-secondary" on:click={close}>Cancel</button>
					<button type="submit" class="btn btn-primary" disabled={submitting}>{submitting ? 'Saving…' : 'Save permissions'}</button>
				</div>
			</form>
		{/if}
	</div>
{/if}
