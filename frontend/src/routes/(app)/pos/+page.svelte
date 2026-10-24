<script lang="ts">
	import { onMount } from 'svelte';
	import { inventoryApi, invoiceApi, customerApi } from '$api/client';
	import { toasts } from '$stores/toasts';

	// Cart state
	let cartItems: {
		product_id: string; sku: string; name: string;
		unit_price: number; gst_rate: number; quantity: number;
		discount_percent: number;
	}[] = [];

	// Search
	let productSearch = '';
	let searchResults: Record<string, unknown>[] = [];
	let searchLoading = false;
	let searchDebounce: ReturnType<typeof setTimeout>;

	// Customer
	let customerSearch = '';
	let customers: Record<string, unknown>[] = [];
	let selectedCustomer: Record<string, unknown> | null = null;
	let showCustomerSearch = false;

	// Payment
	let paymentMethod: 'cash' | 'upi' | 'bank_transfer' | 'razorpay' | 'cheque' = 'cash';
	let amountPaid = '';
	let notes = '';
	let submitting = false;

	// Computed totals
	$: subtotal = cartItems.reduce((sum, item) => {
		const discountMult = 1 - item.discount_percent / 100;
		return sum + item.quantity * item.unit_price * discountMult;
	}, 0);

	$: totalGst = cartItems.reduce((sum, item) => {
		const discountMult = 1 - item.discount_percent / 100;
		const taxable = item.quantity * item.unit_price * discountMult;
		return sum + taxable * item.gst_rate / 100;
	}, 0);

	$: grandTotal = subtotal + totalGst;
	$: outstanding = grandTotal - (parseFloat(amountPaid) || 0);
	$: change = Math.max(0, (parseFloat(amountPaid) || 0) - grandTotal);

	const formatINR = (v: number) => new Intl.NumberFormat('en-IN', { style: 'currency', currency: 'INR', maximumFractionDigits: 2 }).format(v);

	async function searchProducts() {
		if (productSearch.length < 2) { searchResults = []; return; }
		searchLoading = true;
		try {
			const res = await inventoryApi.products.list({ search: productSearch, per_page: 10 }) as { data: Record<string, unknown>[] };
			searchResults = res.data ?? [];
		} catch { searchResults = []; }
		finally { searchLoading = false; }
	}

	function onSearchInput() {
		clearTimeout(searchDebounce);
		searchDebounce = setTimeout(searchProducts, 300);
	}

	function addToCart(product: Record<string, unknown>) {
		const existing = cartItems.find(i => i.product_id === product.id);
		if (existing) {
			existing.quantity++;
			cartItems = [...cartItems];
		} else {
			cartItems = [...cartItems, {
				product_id: String(product.id),
				sku: String(product.sku),
				name: String(product.name),
				unit_price: Number(product.retail_price ?? 0),
				gst_rate: Number(product.gst_rate ?? 18),
				quantity: 1,
				discount_percent: 0
			}];
		}
		productSearch = '';
		searchResults = [];
	}

	function removeFromCart(index: number) {
		cartItems = cartItems.filter((_, i) => i !== index);
	}

	function updateQty(index: number, qty: number) {
		if (qty <= 0) { removeFromCart(index); return; }
		cartItems[index].quantity = qty;
		cartItems = [...cartItems];
	}

	async function searchCustomers() {
		if (customerSearch.length < 2) return;
		try {
			const res = await customerApi.list({ search: customerSearch }) as { data: Record<string, unknown>[] };
			customers = res.data ?? [];
		} catch { customers = []; }
	}

	async function createInvoice() {
		if (cartItems.length === 0) { toasts.warning('Cart is empty'); return; }
		submitting = true;
		try {
			const payload = {
				customer_id: selectedCustomer?.id,
				items: cartItems.map(i => ({
					product_id: i.product_id,
					quantity: String(i.quantity),
					unit_price: String(i.unit_price),
					discount_percent: String(i.discount_percent)
				})),
				amount_paid: amountPaid || '0',
				payment_method: paymentMethod,
				notes
			};
			const res = await invoiceApi.create(payload) as { invoice_number: string; id: string; grand_total: string; outstanding_amount: string };
			toasts.success(`Invoice ${res.invoice_number} created!`, `Total: ${formatINR(Number(res.grand_total))}`);
			// Reset
			cartItems = [];
			selectedCustomer = null;
			amountPaid = '';
			notes = '';
		} catch (e: unknown) {
			const err = e as { message?: string };
			toasts.error('Failed to create invoice', err.message);
		} finally {
			submitting = false;
		}
	}
</script>

<svelte:head>
	<title>POS — RARA Enterprise ERP</title>
</svelte:head>

<div class="h-full flex gap-4 p-6">
	<!-- Left: Product search + cart -->
	<div class="flex-1 flex flex-col gap-4 min-w-0">
		<!-- Header -->
		<div class="flex items-center justify-between">
			<div>
				<h1 class="text-xl font-bold text-slate-900">Point of Sale</h1>
				<p class="text-sm text-slate-500">Add products to create invoice</p>
			</div>
			<div class="flex items-center gap-2">
				<span class="badge badge-primary">{cartItems.length} item{cartItems.length !== 1 ? 's' : ''}</span>
			</div>
		</div>

		<!-- Product search -->
		<div class="relative">
			<div class="relative">
				<svg class="absolute left-4 top-1/2 -translate-y-1/2 text-slate-400 z-10" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/>
				</svg>
				<input
					type="text"
					class="form-control pl-12 py-4 text-base"
					placeholder="Search products by name or SKU (type at least 2 characters)…"
					bind:value={productSearch}
					on:input={onSearchInput}
					autocomplete="off"
				/>
				{#if searchLoading}
					<div class="absolute right-4 top-1/2 -translate-y-1/2 spinner spinner-sm"></div>
				{/if}
			</div>

			<!-- Search results dropdown -->
			{#if searchResults.length > 0}
				<div class="absolute top-full left-0 right-0 z-50 bg-white rounded-xl shadow-xl border border-slate-200 mt-1 max-h-72 overflow-auto">
					{#each searchResults as product}
						{@const p = product as { id: string; name: string; sku: string; retail_price: unknown; gst_rate: unknown; total_stock: unknown; status: string }}
						<button
							class="w-full flex items-center justify-between px-4 py-3 hover:bg-yellow-50 transition-colors text-left border-b border-slate-50 last:border-0"
							on:click={() => addToCart(product)}
						>
							<div>
								<div class="font-medium text-slate-900 text-sm">{p.name}</div>
								<div class="text-xs text-slate-500 font-mono">{p.sku} • GST {p.gst_rate}%</div>
							</div>
							<div class="text-right ml-4">
								<div class="font-bold text-slate-800 text-sm">₹{Number(p.retail_price).toFixed(2)}</div>
								<div class="text-xs {Number(p.total_stock) <= 0 ? 'text-red-500' : 'text-green-600'}">
									{Number(p.total_stock) <= 0 ? 'Out of stock' : `${p.total_stock} in stock`}
								</div>
							</div>
						</button>
					{/each}
				</div>
			{/if}
		</div>

		<!-- Cart items -->
		<div class="card flex-1 overflow-auto">
			{#if cartItems.length === 0}
				<div class="empty-state h-full min-h-48">
					<div class="empty-state-icon">
						<svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<circle cx="9" cy="21" r="1"/><circle cx="20" cy="21" r="1"/>
							<path d="M1 1h4l2.68 13.39a2 2 0 002 1.61h9.72a2 2 0 001.95-1.57l1.65-7.43H6"/>
						</svg>
					</div>
					<h3>Cart is empty</h3>
					<p>Search for products above to add them to the cart.</p>
				</div>
			{:else}
				<table class="data-table">
					<thead>
						<tr>
							<th>Product</th>
							<th class="text-center">Qty</th>
							<th class="text-right">Unit Price</th>
							<th class="text-center">Disc%</th>
							<th class="text-right">GST</th>
							<th class="text-right">Total</th>
							<th></th>
						</tr>
					</thead>
					<tbody>
						{#each cartItems as item, i}
							{@const discMult = 1 - item.discount_percent / 100}
							{@const taxable = item.quantity * item.unit_price * discMult}
							{@const gstAmt = taxable * item.gst_rate / 100}
							{@const lineTotal = taxable + gstAmt}
							<tr>
								<td>
									<div class="font-medium text-sm text-slate-900">{item.name}</div>
									<div class="text-xs text-slate-400 font-mono">{item.sku}</div>
								</td>
								<td class="text-center">
									<div class="flex items-center justify-center gap-1">
										<button class="w-7 h-7 rounded-lg bg-slate-100 hover:bg-slate-200 text-sm font-bold transition-colors flex items-center justify-center"
											on:click={() => updateQty(i, item.quantity - 1)}>−</button>
										<input
											type="number"
											class="w-14 text-center font-mono text-sm py-1 border border-slate-200 rounded-lg focus:outline-none focus:border-yellow-400"
											bind:value={item.quantity}
											on:change={() => updateQty(i, item.quantity)}
											min="1"
										/>
										<button class="w-7 h-7 rounded-lg bg-slate-100 hover:bg-slate-200 text-sm font-bold transition-colors flex items-center justify-center"
											on:click={() => updateQty(i, item.quantity + 1)}>+</button>
									</div>
								</td>
								<td class="text-right">
									<input
										type="number"
										step="0.01"
										class="w-24 text-right font-mono text-sm py-1 border border-slate-200 rounded-lg focus:outline-none focus:border-yellow-400 px-2"
										bind:value={item.unit_price}
									/>
								</td>
								<td class="text-center">
									<input
										type="number"
										step="0.5"
										min="0"
										max="100"
										class="w-16 text-center text-sm py-1 border border-slate-200 rounded-lg focus:outline-none focus:border-yellow-400"
										bind:value={item.discount_percent}
									/>
								</td>
								<td class="text-right text-sm text-slate-500">{formatINR(gstAmt)}</td>
								<td class="text-right font-bold text-sm">{formatINR(lineTotal)}</td>
								<td class="text-right">
									<button class="btn btn-ghost btn-sm text-red-500 hover:text-red-700 hover:bg-red-50"
										on:click={() => removeFromCart(i)}
										aria-label="Remove item from cart">
										<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
											<polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14a2 2 0 01-2 2H8a2 2 0 01-2-2L5 6m3 0V4a1 1 0 011-1h4a1 1 0 011 1v2"/>
										</svg>
									</button>
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			{/if}
		</div>
	</div>

	<!-- Right: Order summary + payment -->
	<div class="w-80 flex-shrink-0 flex flex-col gap-4">
		<!-- Customer selection -->
		<div class="card p-4">
			<h3 class="text-sm font-semibold text-slate-700 mb-3">Customer</h3>
			{#if selectedCustomer}
				<div class="flex items-center gap-2 p-2 bg-yellow-50 rounded-lg border border-yellow-200">
					<div class="w-8 h-8 bg-yellow-400 rounded-full flex items-center justify-center flex-shrink-0">
						<span class="text-sm font-bold text-slate-900">{String(selectedCustomer.name ?? '?')[0].toUpperCase()}</span>
					</div>
					<div class="flex-1 min-w-0">
						<div class="font-medium text-slate-900 text-sm truncate">{selectedCustomer.name}</div>
						<div class="text-xs text-slate-500">{selectedCustomer.phone}</div>
					</div>
					<button class="text-slate-400 hover:text-slate-600" on:click={() => selectedCustomer = null} aria-label="Clear customer selection">
						<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
					</button>
				</div>
			{:else}
				<div class="relative">
					<input
						type="text"
						class="form-control text-sm"
						placeholder="Walk-in customer / search by phone…"
						bind:value={customerSearch}
						on:input={searchCustomers}
						on:focus={() => showCustomerSearch = true}
						on:blur={() => setTimeout(() => showCustomerSearch = false, 200)}
					/>
					{#if showCustomerSearch && customers.length > 0}
						<div class="absolute top-full left-0 right-0 z-50 bg-white rounded-lg shadow-lg border border-slate-200 mt-1 max-h-48 overflow-auto">
							{#each customers as c}
								{@const cust = c as { id: string; name: string; phone: string; outstanding_balance: unknown }}
								<button
									class="w-full flex items-center justify-between px-3 py-2 hover:bg-yellow-50 text-left text-sm"
									on:click={() => { selectedCustomer = c; customerSearch = ''; customers = []; }}
								>
									<div>
										<div class="font-medium text-slate-800">{cust.name}</div>
										<div class="text-xs text-slate-500">{cust.phone}</div>
									</div>
									{#if Number(cust.outstanding_balance) > 0}
										<span class="text-xs text-red-600 font-medium">₹{Number(cust.outstanding_balance).toFixed(0)} due</span>
									{/if}
								</button>
							{/each}
						</div>
					{/if}
				</div>
			{/if}
		</div>

		<!-- Order totals -->
		<div class="card p-4 flex-1">
			<h3 class="text-sm font-semibold text-slate-700 mb-3">Order Summary</h3>
			<div class="space-y-2 text-sm">
				<div class="flex justify-between text-slate-600">
					<span>Subtotal (taxable)</span>
					<span class="font-mono">{formatINR(subtotal)}</span>
				</div>
				<div class="flex justify-between text-slate-600">
					<span>GST</span>
					<span class="font-mono">{formatINR(totalGst)}</span>
				</div>
				<div class="border-t border-slate-200 pt-2 flex justify-between font-bold text-lg text-slate-900">
					<span>Grand Total</span>
					<span class="font-mono text-yellow-700">{formatINR(grandTotal)}</span>
				</div>
			</div>

			<!-- Payment method -->
			<div class="mt-4">
				<label class="form-label text-xs" for="pm">Payment Method</label>
				<div class="grid grid-cols-3 gap-1 mt-1">
					{#each ['cash', 'upi', 'bank_transfer'] as method}
						<button
							class="py-2 px-1 text-xs font-medium rounded-lg border transition-all {paymentMethod === method ? 'bg-yellow-400 border-yellow-500 text-slate-900' : 'bg-white border-slate-200 text-slate-600 hover:border-yellow-300'}"
							on:click={() => paymentMethod = method as typeof paymentMethod}
						>
							{method === 'bank_transfer' ? 'Bank' : method.toUpperCase()}
						</button>
					{/each}
				</div>
			</div>

			<!-- Amount paid -->
			<div class="mt-3">
				<label class="form-label text-xs" for="amount-paid">Amount Paid (₹)</label>
				<input
					id="amount-paid"
					type="number"
					step="0.01"
					class="form-control font-mono text-right"
					placeholder="0.00"
					bind:value={amountPaid}
				/>
				{#if change > 0.01}
					<p class="text-xs text-green-700 font-medium mt-1">Change: {formatINR(change)}</p>
				{:else if outstanding > 0.01 && amountPaid}
					<p class="text-xs text-amber-600 font-medium mt-1">Outstanding: {formatINR(outstanding)}</p>
				{/if}
			</div>

			<!-- Notes -->
			<div class="mt-3">
				<label class="form-label text-xs" for="pos-notes">Notes</label>
				<textarea id="pos-notes" class="form-control text-sm" rows="2" placeholder="Optional notes…" bind:value={notes}></textarea>
			</div>
		</div>

		<!-- Create invoice button -->
		<button
			class="btn btn-primary w-full py-4 text-base font-bold"
			disabled={cartItems.length === 0 || submitting}
			on:click={createInvoice}
		>
			{#if submitting}
				<div class="spinner spinner-sm mr-2"></div>Creating Invoice…
			{:else}
				<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
					<path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z"/><polyline points="14 2 14 8 20 8"/>
					<line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/>
				</svg>
				Create Invoice · {formatINR(grandTotal)}
			{/if}
		</button>
	</div>
</div>
