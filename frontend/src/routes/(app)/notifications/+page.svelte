<script lang="ts">
	import { onMount } from 'svelte';
	import { notificationApi } from '$lib/api/client';
	import { toasts } from '$stores/toasts';

	let notifications: Record<string, unknown>[] = [];
	let unreadCount = 0;
	let loading = true;

	const formatDate = (d: unknown) => d ? new Date(String(d)).toLocaleString('en-IN') : '—';

	async function load() {
		loading = true;
		try {
			const res: any = await notificationApi.list();
			notifications = res.data ?? res ?? [];
			unreadCount = res.unread_count ?? notifications.filter((n) => !n.is_read).length;
		} catch { toasts.error('Failed to load notifications'); }
		loading = false;
	}

	async function markRead(id: string) {
		try { await notificationApi.markRead(id); await load(); }
		catch {}
	}

	async function markAllRead() {
		try { await notificationApi.markAllRead(); toasts.success('All marked as read'); await load(); }
		catch (e: any) { toasts.error(e.message ?? 'Failed'); }
	}

	const TYPE_COLORS: Record<string, string> = { payment_due: 'badge-warning', payment_overdue: 'badge-danger', low_stock: 'badge-warning', system: 'badge-info' };
	onMount(load);
</script>

<svelte:head><title>Notifications — RARA Enterprise ERP</title></svelte:head>

<div class="page-container">
	<div class="page-header">
		<div>
			<h1 class="page-title">Notifications</h1>
			{#if unreadCount > 0}<p class="page-subtitle">{unreadCount} unread</p>{/if}
		</div>
		{#if unreadCount > 0}
			<button class="btn btn-secondary" on:click={markAllRead}>Mark All Read</button>
		{/if}
	</div>

	{#if loading}
		<div class="flex items-center justify-center py-16 text-slate-400">Loading...</div>
	{:else if notifications.length === 0}
		<div class="card"><div class="card-body text-center py-16">
			<p style="font-size:3rem">🔔</p>
			<p class="text-slate-400 text-lg mt-2">No notifications</p>
			<p class="text-slate-500 text-sm">You're all caught up!</p>
		</div></div>
	{:else}
		<div class="flex flex-col gap-2">
		{#each notifications as n}
			<div class="card {!n.is_read ? 'border-l-4 border-yellow-400' : ''}" style="margin-bottom:0;">
				<div class="card-body" style="padding:1rem 1.25rem;">
					<div class="flex items-start justify-between gap-4">
						<div class="flex-1">
							<div class="flex items-center gap-2 mb-1">
								<span class="badge {TYPE_COLORS[String(n.type)] ?? 'badge-secondary'} text-xs">{String(n.type).replace(/_/g,' ')}</span>
								{#if !n.is_read}<span class="w-2 h-2 rounded-full bg-yellow-400 inline-block"></span>{/if}
							</div>
							<p class="font-semibold text-slate-800">{n.title}</p>
							<p class="text-sm text-slate-600 mt-0.5">{n.body}</p>
							<p class="text-xs text-slate-400 mt-1">{formatDate(n.created_at)}</p>
						</div>
						{#if !n.is_read}
							<button class="btn btn-ghost btn-sm text-xs" on:click={() => markRead(String(n.id))}>Mark Read</button>
						{/if}
					</div>
				</div>
			</div>
		{/each}
		</div>
	{/if}
</div>
