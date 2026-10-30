<script lang="ts">
	import { onMount } from 'svelte';
	import { api, authApi, settingsApi, errorMessage } from '$api/client';
	import { currentUser, hasPermission } from '$stores/auth';
	import { toasts } from '$stores/toasts';
	import { humanize } from '$utils/format';

	type Company = {
		name: string; gstin: string; address: string; phone: string; email: string;
		invoice_prefix: string; upi_id: string; upi_name: string; logo_url: string;
	};

	let company: Company = { name: '', gstin: '', address: '', phone: '', email: '', invoice_prefix: 'INV', upi_id: '', upi_name: '', logo_url: '' };
	let companyLoading = true;
	let savingCompany = false;

	let pw = { current: '', next: '', confirm: '' };
	let savingPw = false;
	let health: { status?: string; database?: string; version?: string } = {};

	$: canManage = hasPermission($currentUser, 'settings.manage');
	$: pwMismatch = pw.confirm.length > 0 && pw.next !== pw.confirm;

	async function loadCompany() {
		companyLoading = true;
		try {
			const c = (await settingsApi.company()) as Record<string, string | null>;
			company = Object.fromEntries(Object.entries({ ...company, ...c }).map(([k, v]) => [k, v ?? ''])) as Company;
		} catch (e) {
			toasts.error('Failed to load company settings', errorMessage(e));
		}
		companyLoading = false;
	}

	async function saveCompany() {
		savingCompany = true;
		try {
			await settingsApi.updateCompany(company);
			toasts.success('Company details saved', 'New invoices will use these details');
			await loadCompany();
		} catch (e) { toasts.error('Could not save', errorMessage(e)); }
		savingCompany = false;
	}

	async function changePassword() {
		if (pw.next !== pw.confirm) { toasts.error('New passwords do not match'); return; }
		savingPw = true;
		try {
			const res = await authApi.changePassword(pw.current, pw.next);
			if (res.refresh_token) api.setRefreshToken(res.refresh_token);
			toasts.success('Password changed', res.message);
			pw = { current: '', next: '', confirm: '' };
		} catch (e) { toasts.error('Could not change password', errorMessage(e)); }
		savingPw = false;
	}

	onMount(async () => {
		loadCompany();
		try { health = await api.get('/api/v1/health'); } catch { health = { status: 'unreachable' }; }
	});
</script>

<svelte:head><title>Settings — RARA Enterprise ERP</title></svelte:head>

<div class="page-container">
	<div class="page-header">
		<h1 class="page-title">Settings</h1>
		<p class="page-subtitle">Your account and business configuration</p>
	</div>

	<div class="grid grid-cols-1 xl:grid-cols-2 gap-6">
		<div class="card">
			<div class="card-header"><h2 class="text-lg font-semibold">Company profile</h2></div>
			<form class="card-body space-y-4" on:submit|preventDefault={saveCompany}>
				{#if companyLoading}
					<div class="flex justify-center py-8"><div class="spinner"></div></div>
				{:else}
					<fieldset class="space-y-4" disabled={!canManage}>
						<div class="grid grid-cols-1 md:grid-cols-2 gap-4">
							<div class="form-group md:col-span-2"><label class="form-label required" for="c-name">Business name</label>
								<input id="c-name" class="form-control" bind:value={company.name} required /></div>
							<div class="form-group"><label class="form-label" for="c-gstin">GSTIN</label>
								<input id="c-gstin" class="form-control font-mono uppercase" maxlength="15" bind:value={company.gstin} placeholder="27ABCDE1234F1Z5" />
								<span class="form-hint">Used to decide CGST+SGST vs IGST on invoices.</span></div>
							<div class="form-group"><label class="form-label" for="c-prefix">Invoice prefix</label>
								<input id="c-prefix" class="form-control font-mono uppercase" maxlength="10" bind:value={company.invoice_prefix} />
								<span class="form-hint">e.g. INV → INV-2026-000001</span></div>
							<div class="form-group md:col-span-2"><label class="form-label" for="c-addr">Address</label>
								<textarea id="c-addr" class="form-control" rows="3" bind:value={company.address}></textarea></div>
							<div class="form-group"><label class="form-label" for="c-phone">Phone</label>
								<input id="c-phone" class="form-control" type="tel" bind:value={company.phone} /></div>
							<div class="form-group"><label class="form-label" for="c-email">Email</label>
								<input id="c-email" class="form-control" type="email" bind:value={company.email} /></div>
							<div class="form-group"><label class="form-label" for="c-upi">UPI ID</label>
								<input id="c-upi" class="form-control" bind:value={company.upi_id} placeholder="shop@upi" /></div>
							<div class="form-group"><label class="form-label" for="c-upiname">UPI payee name</label>
								<input id="c-upiname" class="form-control" bind:value={company.upi_name} /></div>
						</div>
					</fieldset>
					{#if canManage}
						<div class="flex justify-end">
							<button type="submit" class="btn btn-primary" disabled={savingCompany}>{savingCompany ? 'Saving…' : 'Save company details'}</button>
						</div>
					{:else}
						<p class="text-sm text-slate-400">Only administrators with Manage Settings permission can edit these details.</p>
					{/if}
				{/if}
			</form>
		</div>

		<div class="space-y-6">
			<div class="card">
				<div class="card-header"><h2 class="text-lg font-semibold">My account</h2></div>
				<div class="card-body">
					<div class="flex items-center gap-4 mb-6">
						<div class="w-14 h-14 rounded-full bg-yellow-400 flex items-center justify-center text-2xl font-bold text-slate-900">
							{$currentUser?.full_name?.charAt(0) ?? '?'}
						</div>
						<div>
							<p class="font-semibold text-lg text-slate-800">{$currentUser?.full_name ?? '—'}</p>
							<p class="text-slate-500">{$currentUser?.email ?? '—'}</p>
							<span class="badge badge-primary mt-1">{humanize($currentUser?.role)}</span>
						</div>
					</div>

					<form class="space-y-3 border-t border-slate-100 pt-4" on:submit|preventDefault={changePassword}>
						<h3 class="font-semibold text-slate-700">Change password</h3>
						<div class="form-group"><label class="form-label" for="pw-cur">Current password</label>
							<input id="pw-cur" class="form-control" type="password" autocomplete="current-password" bind:value={pw.current} required /></div>
						<div class="grid grid-cols-1 md:grid-cols-2 gap-3">
							<div class="form-group"><label class="form-label" for="pw-new">New password</label>
								<input id="pw-new" class="form-control" type="password" autocomplete="new-password" minlength="8" bind:value={pw.next} required />
								<span class="form-hint">At least 8 characters with letters and numbers.</span></div>
							<div class="form-group"><label class="form-label" for="pw-conf">Confirm new password</label>
								<input id="pw-conf" class="form-control {pwMismatch ? 'error' : ''}" type="password" autocomplete="new-password" bind:value={pw.confirm} required />
								{#if pwMismatch}<span class="form-error">Passwords do not match</span>{/if}</div>
						</div>
						<div class="flex justify-end">
							<button type="submit" class="btn btn-primary" disabled={savingPw || pwMismatch}>{savingPw ? 'Updating…' : 'Update password'}</button>
						</div>
					</form>
				</div>
			</div>

			<div class="card">
				<div class="card-header"><h2 class="text-lg font-semibold">Permissions</h2></div>
				<div class="card-body">
					{#if $currentUser?.role === 'super_admin'}
						<p class="text-green-600 font-medium">Full access — Super Administrator</p>
					{:else}
						<div class="flex flex-wrap gap-1">
							{#each $currentUser?.permissions ?? [] as perm}<span class="badge badge-info" style="text-transform:none">{perm}</span>{/each}
						</div>
					{/if}
				</div>
			</div>

			<div class="card">
				<div class="card-header"><h2 class="text-lg font-semibold">System</h2></div>
				<div class="card-body space-y-2 text-sm">
					<div class="flex justify-between"><span class="text-slate-500">Backend version</span><span class="font-mono">{health.version ?? '—'}</span></div>
					<div class="flex justify-between"><span class="text-slate-500">API</span>
						<span class="font-medium {health.status === 'healthy' ? 'text-green-600' : 'text-amber-600'}">● {health.status ?? 'checking…'}</span></div>
					<div class="flex justify-between"><span class="text-slate-500">Database</span>
						<span class="font-medium {health.database === 'connected' ? 'text-green-600' : 'text-red-600'}">● {health.database ?? '—'}</span></div>
				</div>
			</div>
		</div>
	</div>
</div>
