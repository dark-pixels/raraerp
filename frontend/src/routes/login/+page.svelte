<script lang="ts">
	import { goto } from '$app/navigation';
	import { authApi } from '$api/client';
	import { authStore } from '$stores/auth';
	import { toasts } from '$stores/toasts';
	import { z } from 'zod';

	const schema = z.object({
		email: z.string().email('Invalid email address'),
		password: z.string().min(6, 'Password must be at least 6 characters')
	});

	let email = '';
	let password = '';
	let loading = false;
	let errors: Record<string, string> = {};

	async function handleLogin(e: Event) {
		e.preventDefault();
		errors = {};

		const result = schema.safeParse({ email, password });
		if (!result.success) {
			result.error.errors.forEach((err) => {
				if (err.path[0]) errors[err.path[0] as string] = err.message;
			});
			return;
		}

		loading = true;
		try {
			const res = await authApi.login({ email, password });
			import('$api/client').then(({ api }) => api.setTokens(res.access_token, res.refresh_token));
			authStore.setUser(res.user);
			toasts.success('Welcome back!', `Logged in as ${res.user.full_name}`);
			goto('/dashboard');
		} catch (err: unknown) {
			const apiErr = err as { message?: string };
			toasts.error('Login failed', apiErr.message ?? 'Invalid credentials');
			errors.general = apiErr.message ?? 'Invalid credentials';
		} finally {
			loading = false;
		}
	}
</script>

<svelte:head>
	<title>Login — RARA Enterprise ERP</title>
</svelte:head>

<div class="min-h-screen flex items-center justify-center bg-gradient-to-br from-slate-900 via-slate-800 to-slate-900 p-4">
	<!-- Decorative background elements -->
	<div class="absolute inset-0 overflow-hidden pointer-events-none">
		<div class="absolute top-1/4 -left-20 w-96 h-96 bg-yellow-400/5 rounded-full blur-3xl"></div>
		<div class="absolute bottom-1/4 -right-20 w-96 h-96 bg-yellow-400/5 rounded-full blur-3xl"></div>
	</div>

	<div class="w-full max-w-md relative">
		<!-- Logo -->
		<div class="text-center mb-8">
			<div class="inline-flex items-center justify-center w-16 h-16 bg-yellow-400 rounded-2xl shadow-lg mb-4">
				<svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="#0f172a" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
					<path d="M3 9l9-7 9 7v11a2 2 0 01-2 2H5a2 2 0 01-2-2z"/>
					<polyline points="9 22 9 12 15 12 15 22"/>
				</svg>
			</div>
			<h1 class="text-2xl font-bold text-white">RARA Enterprise ERP</h1>
			<p class="text-slate-400 text-sm mt-1">Furniture Hardware Shop Management</p>
		</div>

		<!-- Login Card -->
		<div class="bg-white rounded-2xl shadow-2xl p-8">
			<h2 class="text-xl font-semibold text-slate-900 mb-6">Sign in to your account</h2>

			{#if errors.general}
				<div class="alert alert-danger mb-4">
					<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
						<circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/>
					</svg>
					{errors.general}
				</div>
			{/if}

			<form on:submit={handleLogin} class="space-y-5">
				<div class="form-group">
					<label for="email" class="form-label required">Email address</label>
					<input
						id="email"
						type="email"
						bind:value={email}
						class="form-control {errors.email ? 'error' : ''}"
						placeholder="admin@raraenterprise.com"
						autocomplete="email"
						disabled={loading}
					/>
					{#if errors.email}<p class="form-error">{errors.email}</p>{/if}
				</div>

				<div class="form-group">
					<label for="password" class="form-label required">Password</label>
					<input
						id="password"
						type="password"
						bind:value={password}
						class="form-control {errors.password ? 'error' : ''}"
						placeholder="••••••••"
						autocomplete="current-password"
						disabled={loading}
					/>
					{#if errors.password}<p class="form-error">{errors.password}</p>{/if}
				</div>

				<div class="flex items-center justify-end">
					<a href="/forgot-password" class="text-sm text-yellow-600 hover:text-yellow-700 font-medium">
						Forgot password?
					</a>
				</div>

				<button
					type="submit"
					class="btn btn-primary w-full justify-center py-3 text-base"
					disabled={loading}
				>
					{#if loading}
						<svg class="animate-spin w-4 h-4" viewBox="0 0 24 24" fill="none">
							<circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"/>
							<path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"/>
						</svg>
						Signing in...
					{:else}
						Sign in
					{/if}
				</button>
			</form>

			<div class="mt-6 pt-6 border-t border-slate-100 text-center">
				<p class="text-xs text-slate-400">
					Secure access — unauthorized access is prohibited
				</p>
			</div>
		</div>

		<p class="text-center text-slate-500 text-xs mt-6">
			© 2026 RARA Enterprise. All rights reserved.
		</p>
	</div>
</div>
