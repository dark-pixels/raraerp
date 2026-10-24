import { writable, derived } from 'svelte/store';
import type { AuthUser } from '$types/auth';

// Auth store
function createAuthStore() {
	const { subscribe, set, update } = writable<{
		user: AuthUser | null;
		loading: boolean;
		initialized: boolean;
	}>({
		user: null,
		loading: false,
		initialized: false
	});

	return {
		subscribe,
		setUser: (user: AuthUser | null) =>
			update((s) => ({ ...s, user, initialized: true })),
		setLoading: (loading: boolean) => update((s) => ({ ...s, loading })),
		initialize: (user: AuthUser | null) =>
			update((s) => ({ ...s, user, initialized: true })),
		reset: () => set({ user: null, loading: false, initialized: true })
	};
}

export const authStore = createAuthStore();
export const currentUser = derived(authStore, ($auth) => $auth.user);
export const isAuthenticated = derived(authStore, ($auth) => !!$auth.user);
export const isAuthLoading = derived(authStore, ($auth) => $auth.loading);

export function hasPermission(user: AuthUser | null, permission: string): boolean {
	if (!user) return false;
	if (user.role === 'super_admin') return true;
	return user.permissions.includes(permission);
}
