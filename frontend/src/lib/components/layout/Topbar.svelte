<script lang="ts">
	import { goto } from '$app/navigation';
	import { ui } from '$stores/ui';
	import { authStore, currentUser } from '$stores/auth';
	import { notificationApi, authApi } from '$api/client';
	import { toasts } from '$stores/toasts';
	import { onMount } from 'svelte';

	export let title = '';

	let unreadCount = 0;
	let showUserMenu = false;

	async function loadNotificationCount() {
		try {
			const res = await notificationApi.list() as { unread_count: number };
			unreadCount = res.unread_count ?? 0;
		} catch {}
	}

	async function handleLogout() {
		try {
			await authApi.logout();
		} catch {}
		authStore.reset();
		import('$api/client').then(({ api }) => api.clearTokens());
		goto('/login');
	}

	onMount(loadNotificationCount);
</script>

<header class="topbar">
	<!-- Mobile hamburger -->
	<button
		class="btn btn-ghost lg:hidden"
		on:click={() => ui.openMobileSidebar()}
		aria-label="Toggle sidebar"
	>
		<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
			<line x1="3" y1="6" x2="21" y2="6"/><line x1="3" y1="12" x2="21" y2="12"/><line x1="3" y1="18" x2="21" y2="18"/>
		</svg>
	</button>

	<!-- Page title slot -->
	<div class="flex-1 min-w-0">
		{#if title}
			<h1 class="text-base font-semibold text-slate-800 truncate">{title}</h1>
		{/if}
	</div>

	<!-- Right side actions -->
	<div class="flex items-center gap-2">
		<!-- Notifications -->
		<a href="/notifications" class="relative btn btn-ghost btn-sm" aria-label="Notifications">
			<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
				<path d="M18 8A6 6 0 006 8c0 7-3 9-3 9h18s-3-2-3-9"/>
				<path d="M13.73 21a2 2 0 01-3.46 0"/>
			</svg>
			{#if unreadCount > 0}
				<span class="absolute -top-1 -right-1 min-w-[18px] h-[18px] bg-red-500 text-white text-[10px] font-bold rounded-full flex items-center justify-center px-1">
					{unreadCount > 99 ? '99+' : unreadCount}
				</span>
			{/if}
		</a>

		<!-- User menu -->
		<div class="relative">
			<button
				class="flex items-center gap-2 px-2 py-1.5 rounded-lg hover:bg-slate-100 transition-colors"
				on:click={() => showUserMenu = !showUserMenu}
				on:blur={() => setTimeout(() => showUserMenu = false, 150)}
			>
				<div class="w-7 h-7 bg-yellow-400 rounded-full flex items-center justify-center flex-shrink-0">
					<span class="text-xs font-bold text-slate-900">
						{($currentUser?.full_name ?? 'U')[0].toUpperCase()}
					</span>
				</div>
				<div class="hidden sm:block text-left">
					<div class="text-xs font-semibold text-slate-800 leading-tight">{$currentUser?.full_name ?? '—'}</div>
					<div class="text-[10px] text-slate-500 capitalize">{$currentUser?.role?.replace('_', ' ') ?? ''}</div>
				</div>
				<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
					<polyline points="6 9 12 15 18 9"/>
				</svg>
			</button>

			{#if showUserMenu}
				<div class="absolute right-0 top-full mt-1 w-48 bg-white rounded-xl shadow-xl border border-slate-200 z-50 py-1 overflow-hidden">
					<div class="px-4 py-2.5 border-b border-slate-100">
						<div class="text-sm font-semibold text-slate-900">{$currentUser?.full_name}</div>
						<div class="text-xs text-slate-500">{$currentUser?.email}</div>
					</div>
					<a href="/settings" class="flex items-center gap-2 px-4 py-2.5 text-sm text-slate-700 hover:bg-slate-50 transition-colors">
						<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z"/></svg>
						Settings
					</a>
					<button
						class="w-full flex items-center gap-2 px-4 py-2.5 text-sm text-red-600 hover:bg-red-50 transition-colors"
						on:click={handleLogout}
					>
						<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
							<path d="M9 21H5a2 2 0 01-2-2V5a2 2 0 012-2h4"/><polyline points="16 17 21 12 16 7"/><line x1="21" y1="12" x2="9" y2="12"/>
						</svg>
						Sign out
					</button>
				</div>
			{/if}
		</div>
	</div>
</header>
