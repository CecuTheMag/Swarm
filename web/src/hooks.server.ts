import type { Handle } from '@sveltejs/kit/hooks';

// Every page also says it may not be framed, even by browsers that ignore the
// Content-Security-Policy's frame-ancestors (that policy is `csp` in vite.config.ts), is
// never sniffed into another type, and sends no Referer to other sites, so join and
// invite links stay on this one (spec 0016).
const HEADERS: Record<string, string> = {
	'x-frame-options': 'DENY',
	'x-content-type-options': 'nosniff',
	'referrer-policy': 'same-origin'
};

export const handle: Handle = async ({ event, resolve }) => {
	const response = await resolve(event);
	for (const [name, value] of Object.entries(HEADERS)) response.headers.set(name, value);
	return response;
};
