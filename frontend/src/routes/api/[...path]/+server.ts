import type { RequestHandler } from './$types';

function getBackendUrl(): string {
	return process.env.BACKEND_URL || process.env.PUBLIC_API_BASE_URL || 'http://localhost:8080';
}

const proxyHandler: RequestHandler = async ({ request, params, url }) => {
	const backendUrl = getBackendUrl().replace(/\/$/, '');
	const cleanSubPath = params.path.startsWith('api/') ? params.path.slice(4) : params.path;
	const targetUrl = `${backendUrl}/api/${cleanSubPath}${url.search}`;

	const headers = new Headers(request.headers);
	headers.delete('host');
	headers.delete('connection');

	const hasBody = !['GET', 'HEAD'].includes(request.method);
	const body = hasBody ? await request.arrayBuffer() : undefined;

	try {
		const response = await fetch(targetUrl, {
			method: request.method,
			headers,
			body,
			// @ts-ignore
			duplex: 'half'
		});

		return new Response(response.body, {
			status: response.status,
			statusText: response.statusText,
			headers: response.headers
		});
	} catch (err: any) {
		return new Response(
			JSON.stringify({
				error: 'Proxy Error',
				message: err?.message || 'Failed to communicate with backend service'
			}),
			{
				status: 502,
				headers: { 'Content-Type': 'application/json' }
			}
		);
	}
};

export const GET = proxyHandler;
export const POST = proxyHandler;
export const PUT = proxyHandler;
export const PATCH = proxyHandler;
export const DELETE = proxyHandler;
export const OPTIONS = proxyHandler;
