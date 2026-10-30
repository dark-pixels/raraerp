<script lang="ts">
	import { page } from '$app/stores';
	import { ui } from '$stores/ui';
	import { currentUser } from '$stores/auth';

	const { sidebarCollapsed, mobileSidebarOpen } = ui;

	const NAV_ITEMS = [
		{ section: 'Overview', items: [
			{ href: '/dashboard', icon: 'home', label: 'Dashboard', perm: null }
		]},
		{ section: 'Inventory', items: [
			{ href: '/inventory/products', icon: 'package', label: 'Products', perm: 'inventory.read' },
			{ href: '/inventory/import', icon: 'upload', label: 'Bulk Import', perm: 'inventory.import' },
			{ href: '/racks', icon: 'grid', label: 'Rack Management', perm: 'inventory.read' },
			{ href: '/stock-movements', icon: 'activity', label: 'Stock Movements', perm: 'inventory.read' }
		]},
		{ section: 'Procurement', items: [
			{ href: '/purchases', icon: 'shopping-cart', label: 'Purchases', perm: 'purchases.read' },
			{ href: '/suppliers', icon: 'truck', label: 'Suppliers', perm: 'purchases.read' }
		]},
		{ section: 'Sales', items: [
			{ href: '/pos', icon: 'zap', label: 'POS / Billing', perm: 'sales.create' },
			{ href: '/invoices', icon: 'file-text', label: 'Invoices', perm: 'invoices.create' },
			{ href: '/customers', icon: 'users', label: 'Customers', perm: 'inventory.read' },
			{ href: '/credits', icon: 'credit-card', label: 'Credit Management', perm: 'finance.read' }
		]},
		{ section: 'Finance', items: [
			{ href: '/finance', icon: 'bar-chart-2', label: 'Finance', perm: 'finance.read' },
			{ href: '/payments', icon: 'dollar-sign', label: 'Payments', perm: 'payments.record' },
			{ href: '/gst-verification', icon: 'shield-check', label: 'GST Verification', perm: 'gst.verify' },
			{ href: '/reports', icon: 'pie-chart', label: 'Reports', perm: 'reports.export' }
		]},
		{ section: 'Administration', items: [
			{ href: '/staff', icon: 'user-cog', label: 'Staff Management', perm: 'users.read' },
			{ href: '/notifications', icon: 'bell', label: 'Notifications', perm: null },
			{ href: '/audit-logs', icon: 'list', label: 'Audit Logs', perm: 'settings.manage' },
			{ href: '/settings', icon: 'settings', label: 'Settings', perm: null }
		]}
	];

	const ICONS: Record<string, string> = {
		home: `<path d="M3 9l9-7 9 7v11a2 2 0 01-2 2H5a2 2 0 01-2-2z"/><polyline points="9 22 9 12 15 12 15 22"/>`,
		package: `<line x1="16.5" y1="9.4" x2="7.5" y2="4.21"/><path d="M21 16V8a2 2 0 00-1-1.73l-7-4a2 2 0 00-2 0l-7 4A2 2 0 003 8v8a2 2 0 001 1.73l7 4a2 2 0 002 0l7-4A2 2 0 0021 16z"/><polyline points="3.27 6.96 12 12.01 20.73 6.96"/><line x1="12" y1="22.08" x2="12" y2="12"/>`,
		upload: `<polyline points="16 16 12 12 8 16"/><line x1="12" y1="12" x2="12" y2="21"/><path d="M20.39 18.39A5 5 0 0018 9h-1.26A8 8 0 103 16.3"/>`,
		grid: `<rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/>`,
		activity: `<polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/>`,
		'shopping-cart': `<circle cx="9" cy="21" r="1"/><circle cx="20" cy="21" r="1"/><path d="M1 1h4l2.68 13.39a2 2 0 002 1.61h9.72a2 2 0 001.95-1.57l1.65-7.43H6"/>`,
		truck: `<rect x="1" y="3" width="15" height="13"/><polygon points="16 8 20 8 23 11 23 16 16 16 16 8"/><circle cx="5.5" cy="18.5" r="2.5"/><circle cx="18.5" cy="18.5" r="2.5"/>`,
		zap: `<polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/>`,
		'file-text': `<path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/>`,
		users: `<path d="M17 21v-2a4 4 0 00-4-4H5a4 4 0 00-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M23 21v-2a4 4 0 00-3-3.87"/><path d="M16 3.13a4 4 0 010 7.75"/>`,
		'credit-card': `<rect x="1" y="4" width="22" height="16" rx="2" ry="2"/><line x1="1" y1="10" x2="23" y2="10"/>`,
		'bar-chart-2': `<line x1="18" y1="20" x2="18" y2="10"/><line x1="12" y1="20" x2="12" y2="4"/><line x1="6" y1="20" x2="6" y2="14"/>`,
		'dollar-sign': `<line x1="12" y1="1" x2="12" y2="23"/><path d="M17 5H9.5a3.5 3.5 0 000 7h5a3.5 3.5 0 010 7H6"/>`,
		'shield-check': `<path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/><polyline points="9 12 11 14 15 10"/>`,
		'pie-chart': `<path d="M21.21 15.89A10 10 0 118 2.83"/><path d="M22 12A10 10 0 0012 2v10z"/>`,
		'user-cog': `<circle cx="9" cy="7" r="4"/><path d="M3 21v-2a4 4 0 014-4h4"/><circle cx="19" cy="19" r="3"/>`,
		bell: `<path d="M18 8A6 6 0 006 8c0 7-3 9-3 9h18s-3-2-3-9"/><path d="M13.73 21a2 2 0 01-3.46 0"/>`,
		list: `<line x1="8" y1="6" x2="21" y2="6"/><line x1="8" y1="12" x2="21" y2="12"/><line x1="8" y1="18" x2="21" y2="18"/><line x1="3" y1="6" x2="3.01" y2="6"/><line x1="3" y1="12" x2="3.01" y2="12"/><line x1="3" y1="18" x2="3.01" y2="18"/>`,
		settings: `<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z"/>`
	};

	$: collapsed = $sidebarCollapsed;
	$: currentPath = $page.url.pathname;

	function isActive(href: string, path: string) {
		return path === href || (href !== '/dashboard' && path.startsWith(href + '/'));
	}

	// Recomputed whenever the user loads or changes, so the menu fills in after a refresh.
	$: visibleSections = NAV_ITEMS.map((section) => ({
		...section,
		items: section.items.filter((item) => hasPermission($currentUser, item.perm))
	})).filter((section) => section.items.length > 0);

	function hasPermission(user: typeof $currentUser, perm: string | null): boolean {
		if (!perm) return true;
		if (!user) return false;
		if (user.role === 'super_admin') return true;
		return user.permissions.includes(perm);
	}
</script>

<aside
	class="sidebar {collapsed ? 'collapsed' : ''}"
	class:mobile-open={$mobileSidebarOpen}
>
	<!-- Logo -->
	<div class="sidebar-logo">
		<div class="w-8 h-8 bg-yellow-400 rounded-lg flex items-center justify-center flex-shrink-0">
			<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="#0f172a" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
				<path d="M3 9l9-7 9 7v11a2 2 0 01-2 2H5a2 2 0 01-2-2z"/>
				<polyline points="9 22 9 12 15 12 15 22"/>
			</svg>
		</div>
		{#if !collapsed}
			<div class="overflow-hidden">
				<span class="block text-white font-bold text-sm leading-tight">RARA Enterprise</span>
				<span class="block text-slate-400 text-xs">ERP System</span>
			</div>
		{/if}
	</div>

	<!-- Navigation -->
	<nav class="sidebar-nav">
		{#each visibleSections as section}
			{#each section.items as item, i}
				{#if i === 0 && !collapsed}
					<div class="nav-section-label">{section.section}</div>
				{/if}
				<a
					href={item.href}
					class="nav-item {isActive(item.href, currentPath) ? 'active' : ''}"
					title={collapsed ? item.label : ''}
					on:click={() => ui.closeMobileSidebar()}
				>
					<svg
						class="icon"
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="1.75"
						stroke-linecap="round"
						stroke-linejoin="round"
					>
						{@html ICONS[item.icon] ?? ''}
					</svg>
					{#if !collapsed}
						<span class="truncate">{item.label}</span>
					{/if}
				</a>
			{/each}
		{/each}
	</nav>

	<!-- Footer toggle -->
	<div class="sidebar-footer">
		<button
			class="btn btn-ghost w-full justify-center text-slate-400 hover:text-white"
			on:click={() => ui.toggleSidebar()}
			title={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
			aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
		>
			<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
				{#if collapsed}
					<polyline points="9 18 15 12 9 6"/>
				{:else}
					<polyline points="15 18 9 12 15 6"/>
				{/if}
			</svg>
			{#if !collapsed}
				<span class="text-sm">Collapse</span>
			{/if}
		</button>
	</div>
</aside>
