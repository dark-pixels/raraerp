<script lang="ts">
	import { onMount } from 'svelte';
	import { reportsApi, errorMessage } from '$api/client';
	import { toasts } from '$stores/toasts';
	import { formatINR, formatDate, formatNumber, humanize, isoDate, downloadCsv, toNumber, STATUS_BADGE } from '$utils/format';

	type Row = Record<string, any>;
	type Kind = 'money' | 'date' | 'number' | 'status' | 'text' | 'percent';
	type Col = { key: string; label: string; kind?: Kind };
	type Tab = 'sales' | 'purchases' | 'gst' | 'inventory' | 'receivables' | 'payables';

	const TABS: { key: Tab; label: string; dated: boolean }[] = [
		{ key: 'sales', label: 'Sales', dated: true },
		{ key: 'purchases', label: 'Purchases', dated: true },
		{ key: 'gst', label: 'GST summary', dated: true },
		{ key: 'inventory', label: 'Stock valuation', dated: false },
		{ key: 'receivables', label: 'Receivables', dated: false },
		{ key: 'payables', label: 'Payables', dated: false }
	];

	const COLUMNS: Record<Tab, Col[]> = {
		sales: [
			{ key: 'invoice_number', label: 'Invoice' }, { key: 'date', label: 'Date', kind: 'date' },
			{ key: 'customer', label: 'Customer' }, { key: 'customer_gstin', label: 'GSTIN' },
			{ key: 'taxable_value', label: 'Taxable', kind: 'money' }, { key: 'tax', label: 'GST', kind: 'money' },
			{ key: 'total', label: 'Total', kind: 'money' }, { key: 'paid', label: 'Paid', kind: 'money' },
			{ key: 'outstanding', label: 'Balance', kind: 'money' }, { key: 'status', label: 'Status', kind: 'status' }
		],
		purchases: [
			{ key: 'po_number', label: 'PO' }, { key: 'date', label: 'Date', kind: 'date' },
			{ key: 'supplier', label: 'Supplier' }, { key: 'supplier_invoice', label: 'Supplier inv.' },
			{ key: 'taxable_value', label: 'Taxable', kind: 'money' }, { key: 'gst', label: 'GST', kind: 'money' },
			{ key: 'total', label: 'Total', kind: 'money' }, { key: 'paid', label: 'Paid', kind: 'money' },
			{ key: 'outstanding', label: 'Balance', kind: 'money' }, { key: 'status', label: 'Status', kind: 'status' }
		],
		gst: [
			{ key: 'gst_rate', label: 'GST rate', kind: 'percent' }, { key: 'invoices', label: 'Invoices', kind: 'number' },
			{ key: 'taxable_value', label: 'Taxable value', kind: 'money' }, { key: 'cgst', label: 'CGST', kind: 'money' },
			{ key: 'sgst', label: 'SGST', kind: 'money' }, { key: 'igst', label: 'IGST', kind: 'money' },
			{ key: 'total_tax', label: 'Total tax', kind: 'money' }, { key: 'total', label: 'Invoice value', kind: 'money' }
		],
		inventory: [
			{ key: 'sku', label: 'SKU' }, { key: 'name', label: 'Product' }, { key: 'category', label: 'Category' },
			{ key: 'hsn_code', label: 'HSN' }, { key: 'total_stock', label: 'On hand', kind: 'number' },
			{ key: 'available', label: 'Available', kind: 'number' }, { key: 'reorder_threshold', label: 'Reorder at', kind: 'number' },
			{ key: 'purchase_price', label: 'Cost', kind: 'money' }, { key: 'inventory_value', label: 'Value', kind: 'money' }
		],
		receivables: [
			{ key: 'invoice_number', label: 'Invoice' }, { key: 'customer_name', label: 'Customer' },
			{ key: 'invoice_date', label: 'Date', kind: 'date' }, { key: 'grand_total', label: 'Total', kind: 'money' },
			{ key: 'outstanding_amount', label: 'Outstanding', kind: 'money' }, { key: 'due_date', label: 'Due', kind: 'date' },
			{ key: 'status', label: 'Status', kind: 'status' }
		],
		payables: [
			{ key: 'po_number', label: 'PO' }, { key: 'supplier_name', label: 'Supplier' },
			{ key: 'purchase_date', label: 'Date', kind: 'date' }, { key: 'grand_total', label: 'Total', kind: 'money' },
			{ key: 'outstanding_amount', label: 'Outstanding', kind: 'money' }, { key: 'due_date', label: 'Due', kind: 'date' },
			{ key: 'payment_status', label: 'Status', kind: 'status' }
		]
	};

	let activeTab: Tab = 'sales';
	let rows: Row[] = [];
	let extra: Row = {};
	let loading = false;
	let dateFrom = isoDate(new Date(new Date().getFullYear(), new Date().getMonth(), 1));
	let dateTo = isoDate();
	let lowStockOnly = false;

	$: cols = COLUMNS[activeTab];
	$: dated = TABS.find((t) => t.key === activeTab)?.dated ?? false;
	$: moneyTotals = Object.fromEntries(
		cols.filter((c) => c.kind === 'money' && activeTab !== 'inventory' || c.key === 'inventory_value')
			.map((c) => [c.key, rows.filter((r) => r.status !== 'cancelled').reduce((s, r) => s + toNumber(r[c.key]), 0)])
	);

	async function load() {
		loading = true;
		try {
			const range = dated ? { from: dateFrom, to: dateTo } : {};
			let res: any;
			switch (activeTab) {
				case 'sales': res = await reportsApi.sales(range); break;
				case 'purchases': res = await reportsApi.purchases(range); break;
				case 'gst': res = await reportsApi.gst(range); break;
				case 'inventory': res = await reportsApi.inventory({ low_stock: lowStockOnly || undefined }); break;
				case 'receivables': res = await reportsApi.receivables(); break;
				case 'payables': res = await reportsApi.payables(); break;
			}
			rows = res?.data ?? [];
			extra = res ?? {};
		} catch (e) {
			toasts.error('Failed to load report', errorMessage(e));
			rows = [];
		}
		loading = false;
	}

	function display(row: Row, c: Col): string {
		const v = row[c.key];
		if (v === null || v === undefined || v === '') return '—';
		switch (c.kind) {
			case 'money': return formatINR(v);
			case 'date': return formatDate(v);
			case 'number': return formatNumber(v);
			case 'percent': return `${toNumber(v)}%`;
			case 'status': return humanize(v);
			default: return String(v);
		}
	}

	function exportCsv() {
		const out = rows.map((r) => Object.fromEntries(cols.map((c) => [c.label, c.kind === 'money' || c.kind === 'number' ? toNumber(r[c.key]) : r[c.key] ?? ''])));
		const suffix = dated ? `_${dateFrom}_to_${dateTo}` : `_${isoDate()}`;
		downloadCsv(`${activeTab}-report${suffix}.csv`, out);
	}

	function selectTab(t: Tab) { activeTab = t; rows = []; load(); }

	onMount(load);
</script>

<svelte:head><title>Reports — RARA Enterprise ERP</title></svelte:head>

<div class="page-container">
	<div class="page-header flex items-center justify-between gap-4 flex-wrap">
		<div>
			<h1 class="page-title">Reports</h1>
			<p class="page-subtitle">Sales, purchases, GST, stock and dues</p>
		</div>
		<div class="flex gap-2 no-print">
			<button class="btn btn-secondary" on:click={() => window.print()}>Print</button>
			<button class="btn btn-primary" on:click={exportCsv} disabled={!rows.length}>Export CSV</button>
		</div>
	</div>

	<div class="flex gap-2 mb-4 flex-wrap no-print" role="tablist">
		{#each TABS as t}
			<button role="tab" aria-selected={activeTab === t.key} class="btn {activeTab === t.key ? 'btn-primary' : 'btn-secondary'}" on:click={() => selectTab(t.key)}>{t.label}</button>
		{/each}
	</div>

	{#if dated || activeTab === 'inventory'}
		<div class="card mb-4 no-print">
			<div class="card-body flex gap-4 items-end flex-wrap" style="padding:1rem">
				{#if dated}
					<div class="form-group"><label class="form-label" for="r-from">From</label><input id="r-from" class="input" type="date" bind:value={dateFrom} /></div>
					<div class="form-group"><label class="form-label" for="r-to">To</label><input id="r-to" class="input" type="date" bind:value={dateTo} /></div>
				{:else}
					<label class="flex items-center gap-2 text-sm"><input type="checkbox" bind:checked={lowStockOnly} /> Low stock only</label>
				{/if}
				<button class="btn btn-primary" on:click={load}>Apply</button>
			</div>
		</div>
	{/if}

	{#if activeTab === 'gst' && !loading && rows.length}
		<div class="grid grid-cols-1 md:grid-cols-3 gap-4 mb-4">
			<div class="card card-body"><p class="text-xs uppercase tracking-widest text-slate-400 font-semibold">Output tax (sales)</p><p class="text-2xl font-bold">{formatINR(extra.output_tax)}</p></div>
			<div class="card card-body"><p class="text-xs uppercase tracking-widest text-slate-400 font-semibold">Input tax (purchases)</p><p class="text-2xl font-bold">{formatINR(extra.input_tax)}</p></div>
			<div class="card card-body"><p class="text-xs uppercase tracking-widest text-slate-400 font-semibold">Net tax payable</p><p class="text-2xl font-bold {toNumber(extra.net_tax_payable) > 0 ? 'text-red-600' : 'text-green-600'}">{formatINR(extra.net_tax_payable)}</p></div>
		</div>
	{/if}

	{#if loading}
		<div class="flex items-center justify-center py-16 gap-3 text-slate-400"><div class="spinner"></div> Loading report…</div>
	{:else if rows.length === 0}
		<div class="card"><div class="card-body text-center py-16"><p class="text-slate-400 text-lg">No data for the selected period</p></div></div>
	{:else}
		<div class="card">
			<div class="table-container">
				<table class="table">
					<thead><tr>{#each cols as c}<th class={c.kind === 'money' || c.kind === 'number' ? 'text-right' : ''}>{c.label}</th>{/each}</tr></thead>
					<tbody>
						{#each rows as row}
							<tr class={row.status === 'cancelled' ? 'opacity-50' : ''}>
								{#each cols as c}
									<td class="text-sm {c.kind === 'money' || c.kind === 'number' ? 'text-right font-mono' : ''}">
										{#if c.kind === 'status'}
											<span class="badge {STATUS_BADGE[row[c.key]] ?? 'badge-secondary'}">{display(row, c)}</span>
										{:else}
											{display(row, c)}
										{/if}
									</td>
								{/each}
							</tr>
						{/each}
					</tbody>
					<tfoot>
						<tr class="font-semibold bg-slate-50">
							{#each cols as c, i}
								<td class="text-sm {c.kind === 'money' ? 'text-right font-mono' : ''}" style="padding:0.75rem 1rem">
									{#if i === 0}Total ({rows.length}){:else if c.key in moneyTotals}{formatINR(moneyTotals[c.key])}{/if}
								</td>
							{/each}
						</tr>
					</tfoot>
				</table>
			</div>
		</div>
		{#if activeTab === 'gst' && extra.hsn_summary?.length}
			<div class="card mt-4">
				<div class="card-header"><h2 class="font-semibold">HSN-wise summary</h2></div>
				<div class="table-container">
					<table class="table">
						<thead><tr><th>HSN</th><th class="text-right">GST rate</th><th class="text-right">Quantity</th><th class="text-right">Taxable value</th><th class="text-right">Tax</th></tr></thead>
						<tbody>
							{#each extra.hsn_summary as h}
								<tr><td class="font-mono">{h.hsn_code}</td><td class="text-right">{toNumber(h.gst_rate)}%</td><td class="text-right">{formatNumber(h.quantity)}</td><td class="text-right">{formatINR(h.taxable_value)}</td><td class="text-right">{formatINR(h.tax)}</td></tr>
							{/each}
						</tbody>
					</table>
				</div>
			</div>
		{/if}
		<p class="text-xs text-slate-400 mt-3">Cancelled documents are shown faded and excluded from totals.</p>
	{/if}
</div>
