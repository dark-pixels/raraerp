const inr = new Intl.NumberFormat('en-IN', { style: 'currency', currency: 'INR', maximumFractionDigits: 2 });
const inrWhole = new Intl.NumberFormat('en-IN', { style: 'currency', currency: 'INR', maximumFractionDigits: 0 });

export function toNumber(v: unknown): number {
	const n = typeof v === 'number' ? v : parseFloat(String(v ?? 0));
	return Number.isFinite(n) ? n : 0;
}

export const formatINR = (v: unknown) => inr.format(toNumber(v));
export const formatINRWhole = (v: unknown) => inrWhole.format(toNumber(v));
export const formatNumber = (v: unknown) => toNumber(v).toLocaleString('en-IN', { maximumFractionDigits: 3 });

export function formatDate(d: unknown): string {
	if (!d) return '—';
	const date = new Date(String(d));
	return Number.isNaN(date.getTime())
		? '—'
		: date.toLocaleDateString('en-IN', { day: '2-digit', month: 'short', year: 'numeric' });
}

export function formatDateTime(d: unknown): string {
	if (!d) return '—';
	const date = new Date(String(d));
	return Number.isNaN(date.getTime())
		? '—'
		: date.toLocaleString('en-IN', { day: '2-digit', month: 'short', year: 'numeric', hour: '2-digit', minute: '2-digit' });
}

export const humanize = (s: unknown) => String(s ?? '').replace(/_/g, ' ');

/** Today's date as YYYY-MM-DD in local time (for date inputs). */
export function isoDate(d = new Date()): string {
	const off = d.getTimezoneOffset();
	return new Date(d.getTime() - off * 60_000).toISOString().slice(0, 10);
}

export function debounce<A extends unknown[]>(fn: (...args: A) => void, ms = 300) {
	let t: ReturnType<typeof setTimeout> | undefined;
	return (...args: A) => {
		clearTimeout(t);
		t = setTimeout(() => fn(...args), ms);
	};
}

/** Downloads rows as a CSV file (Excel-compatible, UTF-8 with BOM). */
export function downloadCsv(filename: string, rows: Record<string, unknown>[]) {
	if (rows.length === 0) return;
	const headers = Object.keys(rows[0]);
	const escape = (v: unknown) => {
		const s = v === null || v === undefined ? '' : typeof v === 'object' ? JSON.stringify(v) : String(v);
		return /[",\n\r]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
	};
	const csv = [headers.join(','), ...rows.map((r) => headers.map((h) => escape(r[h])).join(','))].join('\r\n');
	const blob = new Blob(['﻿' + csv], { type: 'text/csv;charset=utf-8' });
	const url = URL.createObjectURL(blob);
	const a = document.createElement('a');
	a.href = url;
	a.download = filename;
	a.click();
	setTimeout(() => URL.revokeObjectURL(url), 10_000);
}

export const STATUS_BADGE: Record<string, string> = {
	draft: 'badge-warning',
	issued: 'badge-info',
	approved: 'badge-info',
	ordered: 'badge-primary',
	partially_received: 'badge-warning',
	received: 'badge-success',
	partially_paid: 'badge-warning',
	unpaid: 'badge-danger',
	paid: 'badge-success',
	overdue: 'badge-danger',
	cancelled: 'badge-secondary',
	returned: 'badge-secondary',
	pending: 'badge-warning',
	rejected: 'badge-danger',
	verified: 'badge-success',
	failed: 'badge-danger',
	completed: 'badge-success',
	processing: 'badge-info'
};
