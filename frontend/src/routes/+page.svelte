<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { api } from '$api/client';

	onMount(() => {
		goto(api.hasSession() ? '/dashboard' : '/login', { replaceState: true });
	});
</script>

<svelte:head><title>RARA Enterprise ERP</title></svelte:head>

<div class="flex h-screen items-center justify-center bg-slate-50">
	<div class="spinner"></div>
</div>
