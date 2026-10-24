<script lang="ts">
	import { ui, sidebarWidth } from '$stores/ui';
	import { currentUser } from '$stores/auth';
	import { page } from '$app/stores';
	import Sidebar from '$components/layout/Sidebar.svelte';
	import Topbar from '$components/layout/Topbar.svelte';

	// Individual store subscriptions
	const { sidebarCollapsed, mobileSidebarOpen } = ui;
</script>

<div class="flex h-screen overflow-hidden bg-slate-50">
	<Sidebar />

	<!-- Main area -->
	<div class="flex-1 flex flex-col min-w-0 overflow-hidden" style="margin-left: {$sidebarWidth}px; transition: margin-left 200ms ease;">
		<Topbar />
		<main class="flex-1 overflow-y-auto pt-16">
			<slot />
		</main>
	</div>

	<!-- Mobile overlay -->
	{#if $mobileSidebarOpen}
		<div
			class="fixed inset-0 bg-black/40 z-20 lg:hidden"
			on:click={() => ui.closeMobileSidebar()}
			role="presentation"
		></div>
	{/if}
</div>
