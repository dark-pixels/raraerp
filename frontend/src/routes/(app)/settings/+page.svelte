<script lang="ts">
	import { currentUser } from '$stores/auth';
	import { toasts } from '$stores/toasts';

	let user = $currentUser;
	const version = '1.0.0';
	const buildDate = '24 Oct 2026';
</script>

<svelte:head><title>Settings — RARA Enterprise ERP</title></svelte:head>

<div class="page-container">
	<div class="page-header">
		<h1 class="page-title">Settings</h1>
	</div>

	<div class="grid grid-cols-2 gap-6">
		<div class="card">
			<div class="card-header"><h2 class="text-lg font-semibold">Account</h2></div>
			<div class="card-body">
				<div class="flex items-center gap-4 mb-6">
					<div class="w-16 h-16 rounded-full bg-yellow-400 flex items-center justify-center text-2xl font-bold text-slate-900">
						{user?.full_name?.charAt(0) ?? 'A'}
					</div>
					<div>
						<p class="font-semibold text-lg text-slate-800">{user?.full_name ?? '—'}</p>
						<p class="text-slate-500">{user?.email ?? '—'}</p>
						<span class="badge badge-primary mt-1">{String(user?.role ?? '').replace('_',' ')}</span>
					</div>
				</div>
				<div class="border-t border-slate-100 pt-4">
					<p class="text-sm text-slate-500 mb-3">To change your password, contact your system administrator.</p>
				</div>
			</div>
		</div>

		<div class="card">
			<div class="card-header"><h2 class="text-lg font-semibold">System Info</h2></div>
			<div class="card-body">
				<div class="space-y-3">
					<div class="flex justify-between py-2 border-b border-slate-100">
						<span class="text-slate-500">Application</span>
						<span class="font-medium">RARA Enterprise ERP</span>
					</div>
					<div class="flex justify-between py-2 border-b border-slate-100">
						<span class="text-slate-500">Version</span>
						<span class="font-mono">{version}</span>
					</div>
					<div class="flex justify-between py-2 border-b border-slate-100">
						<span class="text-slate-500">Build Date</span>
						<span>{buildDate}</span>
					</div>
					<div class="flex justify-between py-2 border-b border-slate-100">
						<span class="text-slate-500">Backend API</span>
						<span class="text-green-600 font-medium">● Connected</span>
					</div>
					<div class="flex justify-between py-2">
						<span class="text-slate-500">Database</span>
						<span class="text-green-600 font-medium">● PostgreSQL (Aiven)</span>
					</div>
				</div>
			</div>
		</div>

		<div class="card">
			<div class="card-header"><h2 class="text-lg font-semibold">Business Configuration</h2></div>
			<div class="card-body">
				<div class="space-y-3">
					<div class="flex justify-between py-2 border-b border-slate-100">
						<span class="text-slate-500">Business Name</span><span class="font-medium">RARA Enterprise</span>
					</div>
					<div class="flex justify-between py-2 border-b border-slate-100">
						<span class="text-slate-500">Currency</span><span class="font-medium">INR (₹)</span>
					</div>
					<div class="flex justify-between py-2 border-b border-slate-100">
						<span class="text-slate-500">Timezone</span><span>Asia/Kolkata (IST)</span>
					</div>
					<div class="flex justify-between py-2">
						<span class="text-slate-500">GST Registered</span><span class="text-green-600">Yes</span>
					</div>
				</div>
			</div>
		</div>

		<div class="card">
			<div class="card-header"><h2 class="text-lg font-semibold">Permissions</h2></div>
			<div class="card-body">
				{#if user?.role === 'super_admin'}
					<p class="text-green-600 font-medium mb-3">✓ Full access — Super Administrator</p>
					<p class="text-sm text-slate-500">You have unrestricted access to all modules and settings.</p>
				{:else}
					<div class="space-y-2">
					{#each user?.permissions ?? [] as perm}
						<span class="badge badge-info mr-1">{perm}</span>
					{/each}
					{#if !user?.permissions?.length}
						<p class="text-slate-400 text-sm">No custom permissions assigned. Using role defaults.</p>
					{/if}
					</div>
				{/if}
			</div>
		</div>
	</div>
</div>
