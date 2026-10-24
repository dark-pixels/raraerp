import { writable, derived } from 'svelte/store';

const notifCount = writable(0);
const sidebarCollapsed = writable(false);
const mobileSidebarOpen = writable(false);

export const ui = {
	notifCount,
	sidebarCollapsed,
	mobileSidebarOpen,
	toggleSidebar: () => sidebarCollapsed.update((v) => !v),
	openMobileSidebar: () => mobileSidebarOpen.set(true),
	closeMobileSidebar: () => mobileSidebarOpen.set(false),
	setNotifCount: (n: number) => notifCount.set(n)
};

export const sidebarWidth = derived(
	sidebarCollapsed,
	($collapsed) => ($collapsed ? 64 : 256)
);
