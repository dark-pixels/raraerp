import { writable } from 'svelte/store';

export type ToastType = 'success' | 'error' | 'warning' | 'info';

export interface Toast {
	id: string;
	type: ToastType;
	message: string;
	detail?: string;
	duration?: number;
}

function createToastStore() {
	const { subscribe, update } = writable<Toast[]>([]);

	function add(toast: Omit<Toast, 'id'>) {
		const id = crypto.randomUUID();
		update((toasts) => [...toasts, { ...toast, id }]);
		const duration = toast.duration ?? 4500;
		if (duration > 0) {
			setTimeout(() => dismiss(id), duration);
		}
		return id;
	}

	function dismiss(id: string) {
		update((toasts) => toasts.filter((t) => t.id !== id));
	}

	return {
		subscribe,
		success: (message: string, detail?: string) =>
			add({ type: 'success', message, detail }),
		error: (message: string, detail?: string) =>
			add({ type: 'error', message, detail, duration: 7000 }),
		warning: (message: string, detail?: string) =>
			add({ type: 'warning', message, detail }),
		info: (message: string, detail?: string) =>
			add({ type: 'info', message, detail }),
		dismiss
	};
}

export const toasts = createToastStore();
