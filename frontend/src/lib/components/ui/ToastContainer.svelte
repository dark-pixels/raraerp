<script lang="ts">
	import { toasts, type Toast } from '$stores/toasts';

	function dismiss(id: string) {
		toasts.dismiss(id);
	}

	const ICONS = {
		success: `<polyline points="20 6 9 17 4 12"/>`,
		error: `<line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>`,
		warning: `<path d="M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/>`,
		info: `<circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/>`
	};

	const COLORS = {
		success: '#22c55e',
		error: '#ef4444',
		warning: '#f59e0b',
		info: '#3b82f6'
	};
</script>

<div class="toast-container" role="region" aria-label="Notifications" aria-live="polite">
	{#each $toasts as toast (toast.id)}
		<div class="toast" role="alert">
			<div class="toast-icon" style="color: {COLORS[toast.type]}">
				<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
					{@html ICONS[toast.type]}
				</svg>
			</div>
			<div class="flex-1 min-w-0">
				<p class="text-sm font-semibold leading-tight">{toast.message}</p>
				{#if toast.detail}
					<p class="text-xs text-slate-400 mt-0.5 leading-tight truncate">{toast.detail}</p>
				{/if}
			</div>
			<button
				class="text-slate-500 hover:text-slate-300 transition-colors flex-shrink-0 ml-2"
				on:click={() => dismiss(toast.id)}
				aria-label="Dismiss notification"
			>
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
				</svg>
			</button>
		</div>
	{/each}
</div>

<style>
	.toast-container {
		position: fixed;
		top: 1rem;
		right: 1rem;
		z-index: 9999;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		max-width: 360px;
		pointer-events: none;
	}
	.toast {
		pointer-events: all;
		display: flex;
		align-items: flex-start;
		gap: 0.75rem;
		padding: 0.875rem 1rem;
		background: #1e293b;
		color: #f8fafc;
		border-radius: 10px;
		box-shadow: 0 10px 30px rgba(0,0,0,.25);
		animation: toastIn .25s cubic-bezier(.34,1.56,.64,1);
		border-left: 3px solid transparent;
	}
	.toast-icon { flex-shrink: 0; margin-top: 1px; }
	@keyframes toastIn {
		from { opacity: 0; transform: translateX(100%) scale(.95); }
		to   { opacity: 1; transform: translateX(0) scale(1); }
	}
</style>
