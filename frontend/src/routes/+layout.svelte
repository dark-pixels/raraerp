<script lang="ts">
	import '../app.css';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import { authStore, isAuthenticated } from '$stores/auth';
	import { authApi } from '$api/client';
	import ToastContainer from '$components/ui/ToastContainer.svelte';

	const PUBLIC_ROUTES = ['/login', '/forgot-password'];

	onMount(async () => {
		const path = $page.url.pathname;
		const isPublic = PUBLIC_ROUTES.some((r) => path.startsWith(r));

		try {
			const user = await authApi.me();
			authStore.initialize(user);
			if (path === '/login') goto('/dashboard');
		} catch {
			authStore.initialize(null);
			if (!isPublic) goto('/login');
		}
	});
</script>

<ToastContainer />
<slot />
