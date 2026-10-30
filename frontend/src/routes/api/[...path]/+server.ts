import type { RequestHandler } from './$types';

function getBackendUrl(): string {
	return process.env.BACKEND_URL || process.env.PUBLIC_API_BASE_URL || 'http://localhost:8080';
}

// Hop-by-hop headers, plus encoding/length headers that no longer match once
// Node's fetch has transparently decompressed the upstream body.
const STRIP_RESPONSE_HEADERS = ['content-encoding', 'content-length', 'transfer-encoding', 'connection', 'keep-alive'];

const proxyHandler: RequestHandler = async ({ request, params, url }) => {
	const backendUrl = getBackendUrl().replace(/\/$/, '');
	const subPath = params.path.startsWith('api/') ? params.path.slice(4) : params.path;
	const targetUrl = `${backendUrl}/api/${subPath}${url.search}`;

	const headers = new Headers(request.headers);
	for (const h of ['host', 'connection', 'content-length', 'accept-encoding']) headers.delete(h);

	const hasBody = !['GET', 'HEAD'].includes(request.method);
	const body = hasBody ? await request.arrayBuffer() : undefined;

	try {
		const response = await fetch(targetUrl, {
			method: request.method,
			headers,
			body,
			redirect: 'manual'
		});

		const outHeaders = new Headers(response.headers);
		for (const h of STRIP_RESPONSE_HEADERS) outHeaders.delete(h);

		return new Response(response.body, {
			status: response.status,
			statusText: response.statusText,
			headers: outHeaders
		});
	} catch (err) {
		console.error(`API proxy to ${targetUrl} failed:`, err);
		return new Response(
			JSON.stringify({
				error: 'backend_unreachable',
				message: 'The server is not reachable right now. Please try again in a moment.'
			}),
			{ status: 502, headers: { 'Content-Type': 'application/json' } }
		);
	}
};

export const GET = proxyHandler;
export const POST = proxyHandler;
export const PUT = proxyHandler;
export const PATCH = proxyHandler;
export const DELETE = proxyHandler;
export const OPTIONS = proxyHandler;
