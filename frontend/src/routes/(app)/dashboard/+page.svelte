<script lang="ts">
	import { onMount } from 'svelte';
	import { dashboardApi } from '$api/client';
	import { toasts } from '$stores/toasts';
	import { currentUser } from '$stores/auth';

	let summary: Record<string, unknown> = {};
	let trend: { date: string; total: string }[] = [];
	let activity: unknown[] = [];
	let loading = true;

	const formatINR = (val: unknown) => {
		const num = parseFloat(String(val ?? 0));
		return new Intl.NumberFormat('en-IN', { style: 'currency', currency: 'INR', maximumFractionDigits: 0 }).format(num);
	};

	const formatNum = (val: unknown) => Number(val ?? 0).toLocaleString('en-IN');

	onMount(async () => {
		try {
			const [s, t, a] = await Promise.all([
				dashboardApi.summary(),
				dashboardApi.salesTrend(),
				dashboardApi.activity()
			]);
			summary = s as Record<string, unknown>;
			trend = ((t as {data: {date: string; total: string}[]}).data ?? []);
			activity = ((a as {data: unknown[]}).data ?? []);
		} catch (e: unknown) {
			const apiErr = e as { message?: string };
			toasts.error('Failed to load dashboard', apiErr.message);
		} finally {
			loading = false;
		}
	});

	const STAT_CARDS = [
		{ label: 'Today Sales', key: 'today_sales', format: 'currency', color: '#16a34a', bg: '#dcfce7', icon: 'trending-up' },
		{ label: 'Monthly Sales', key: 'monthly_sales', format: 'currency', color: '#2563eb', bg: '#dbeafe', icon: 'bar-chart' },
		{ label: 'Receivables', key: 'customer_receivables', format: 'currency', color: '#d97706', bg: '#fef3c7', icon: 'credit-card' },
		{ label: 'Payables', key: 'supplier_payables', format: 'currency', color: '#7c3aed', bg: '#ede9fe', icon: 'package' },
		{ label: 'Today Collected', key: 'today_collected', format: 'currency', color: '#0891b2', bg: '#cffafe', icon: 'check-circle' },
		{ label: 'Low Stock Items', key: 'low_stock_count', format: 'number', color: '#dc2626', bg: '#fee2e2', icon: 'alert-triangle' },
		{ label: 'Overdue Invoices', key: 'overdue_customer_invoices', format: 'number', color: '#dc2626', bg: '#fee2e2', icon: 'clock' },
		{ label: 'Total Products', key: 'total_products', format: 'number', color: '#374151', bg: '#f1f5f9', icon: 'box' },
	];
</script>

<svelte:head>
	<title>Dashboard — RARA Enterprise ERP</title>
</svelte:head>

<div class="page-container">
	<!-- Page header -->
	<div class="page-header flex items-center justify-between">
		<div>
			<h1 class="page-title">Dashboard</h1>
			<p class="page-subtitle">
				Welcome back, <strong>{$currentUser?.full_name ?? 'User'}</strong> •
				{new Date().toLocaleDateString('en-IN', { weekday: 'long', year: 'numeric', month: 'long', day: 'numeric' })}
			</p>
		</div>
		<div class="flex gap-2">
			<select class="form-control w-auto">
				<option>Today</option>
				<option>Last 7 days</option>
				<option>This month</option>
				<option>Last month</option>
				<option>Custom range</option>
			</select>
			<button class="btn btn-primary">
				<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
					<polyline points="6 9 12 3 18 9"/><line x1="12" y1="3" x2="12" y2="15"/>
					<path d="M5 20h14a1 1 0 001-1v-3"/>
				</svg>
				Export
			</button>
		</div>
	</div>

	{#if loading}
		<!-- Loading skeletons -->
		<div class="grid grid-cols-2 lg:grid-cols-4 gap-4 mb-6">
			{#each Array(8) as _}
				<div class="card p-5">
					<div class="skeleton h-3 w-24 mb-3"></div>
					<div class="skeleton h-8 w-32 mb-2"></div>
					<div class="skeleton h-3 w-16"></div>
				</div>
			{/each}
		</div>
	{:else}
		<!-- KPI cards -->
		<div class="grid grid-cols-2 lg:grid-cols-4 gap-4 mb-6">
			{#each STAT_CARDS as card}
				<div class="stat-card">
					<div class="stat-icon" style="background: {card.bg}; color: {card.color}">
						<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
							{#if card.icon === 'trending-up'}
								<polyline points="23 6 13.5 15.5 8.5 10.5 1 18"/><polyline points="17 6 23 6 23 12"/>
							{:else if card.icon === 'bar-chart'}
								<line x1="18" y1="20" x2="18" y2="10"/><line x1="12" y1="20" x2="12" y2="4"/><line x1="6" y1="20" x2="6" y2="14"/>
							{:else if card.icon === 'credit-card'}
								<rect x="1" y="4" width="22" height="16" rx="2" ry="2"/><line x1="1" y1="10" x2="23" y2="10"/>
							{:else if card.icon === 'package'}
								<line x1="16.5" y1="9.4" x2="7.5" y2="4.21"/><path d="M21 16V8a2 2 0 00-1-1.73l-7-4a2 2 0 00-2 0l-7 4A2 2 0 003 8v8a2 2 0 001 1.73l7 4a2 2 0 002 0l7-4A2 2 0 0021 16z"/>
							{:else if card.icon === 'check-circle'}
								<path d="M22 11.08V12a10 10 0 11-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/>
							{:else if card.icon === 'alert-triangle'}
								<path d="M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/>
							{:else if card.icon === 'clock'}
								<circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/>
							{:else}
								<path d="M21 16V8a2 2 0 00-1-1.73l-7-4a2 2 0 00-2 0l-7 4A2 2 0 003 8v8a2 2 0 001 1.73l7 4a2 2 0 002 0l7-4A2 2 0 0021 16z"/>
							{/if}
						</svg>
					</div>
					<div class="stat-label">{card.label}</div>
					<div class="stat-value" style="color: {card.color}">
						{card.format === 'currency' ? formatINR(summary[card.key]) : formatNum(summary[card.key])}
					</div>
				</div>
			{/each}
		</div>

		<!-- Charts + Activity row -->
		<div class="grid grid-cols-1 lg:grid-cols-3 gap-5 mb-6">
			<!-- Sales trend chart -->
			<div class="card lg:col-span-2">
				<div class="card-header">
					<h3 class="text-base font-semibold text-slate-900">Sales Trend (Last 30 Days)</h3>
				</div>
				<div class="card-body">
					{#if trend.length === 0}
						<div class="empty-state py-12">
							<div class="empty-state-icon">
								<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg>
							</div>
							<h3>No sales data</h3>
							<p>Sales will appear here once invoices are created.</p>
						</div>
					{:else}
						<!-- Simple bar chart using CSS -->
						{@const maxVal = Math.max(...trend.map(t => parseFloat(t.total) || 0), 1)}
						<div class="flex items-end gap-1 h-40 px-2">
							{#each trend as item}
								{@const height = Math.max((parseFloat(item.total) / maxVal) * 100, 2)}
								<div class="flex-1 flex flex-col items-center gap-1 group" title="{item.date}: {formatINR(item.total)}">
									<div
										class="w-full rounded-t-sm transition-all duration-200 group-hover:opacity-80 cursor-pointer"
										style="height: {height}%; background: linear-gradient(to top, #eab308, #facc15);"
									></div>
								</div>
							{/each}
						</div>
						<div class="flex justify-between text-xs text-slate-400 mt-2 px-2">
							<span>{trend[0]?.date ?? ''}</span>
							<span>{trend[trend.length - 1]?.date ?? ''}</span>
						</div>
					{/if}
				</div>
			</div>

			<!-- Activity feed -->
			<div class="card">
				<div class="card-header">
					<h3 class="text-base font-semibold text-slate-900">Recent Activity</h3>
				</div>
				<div class="card-body p-0">
					{#if activity.length === 0}
						<div class="empty-state py-8">
							<h3>No activity yet</h3>
							<p>Recent transactions will appear here.</p>
						</div>
					{:else}
						<div class="divide-y divide-slate-50">
							{#each activity.slice(0, 8) as item}
								{@const a = item as {type: string; reference: string; amount: string; timestamp: string}}
								<div class="flex items-center gap-3 p-3 hover:bg-slate-50 transition-colors">
									<div class="w-8 h-8 rounded-full flex items-center justify-center flex-shrink-0
										{a.type === 'invoice' ? 'bg-green-100 text-green-600' : 'bg-blue-100 text-blue-600'}">
										<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
											{#if a.type === 'invoice'}
												<path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z"/>
											{:else}
												<circle cx="9" cy="21" r="1"/><circle cx="20" cy="21" r="1"/><path d="M1 1h4l2.68 13.39a2 2 0 002 1.61h9.72a2 2 0 001.95-1.57l1.65-7.43H6"/>
											{/if}
										</svg>
									</div>
									<div class="flex-1 min-w-0">
										<p class="text-sm font-medium text-slate-800 truncate">{a.reference}</p>
										<p class="text-xs text-slate-400">{a.type === 'invoice' ? 'Invoice' : 'Purchase'}</p>
									</div>
									<div class="text-sm font-semibold text-slate-700">{formatINR(a.amount)}</div>
								</div>
							{/each}
						</div>
					{/if}
				</div>
			</div>
		</div>

		<!-- Quick actions -->
		<div class="grid grid-cols-2 md:grid-cols-4 gap-4">
			{#each [
				{ label: 'New Invoice', href: '/pos', color: '#16a34a', bg: '#dcfce7' },
				{ label: 'New Purchase', href: '/purchases', color: '#2563eb', bg: '#dbeafe' },
				{ label: 'Add Product', href: '/inventory/products', color: '#7c3aed', bg: '#ede9fe' },
				{ label: 'Stock Report', href: '/reports', color: '#d97706', bg: '#fef3c7' },
			] as action}
				<a
					href={action.href}
					class="card p-4 flex items-center gap-3 hover:shadow-md transition-all duration-200 no-underline group"
					style="border-left: 4px solid {action.color}"
				>
					<div class="w-9 h-9 rounded-lg flex items-center justify-center flex-shrink-0 transition-transform group-hover:scale-110"
						style="background: {action.bg}; color: {action.color}">
						<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>
						</svg>
					</div>
					<span class="font-semibold text-sm text-slate-700 group-hover:text-slate-900">{action.label}</span>
				</a>
			{/each}
		</div>
	{/if}
</div>
