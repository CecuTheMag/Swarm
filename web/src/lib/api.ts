import type { ApiError } from '#lib/protocol/ApiError.ts';

/** A failed API call: the HTTP status and the coordinator's message. */
export class ApiFailure extends Error {
	constructor(
		readonly status: number,
		message: string
	) {
		super(message);
	}
}

/**
 * Calls the coordinator's REST API on this site's origin with the session cookie.
 * Returns the parsed body (or undefined for 204), throws `ApiFailure` otherwise.
 */
export async function api<T = unknown>(
	path: string,
	init: { method?: string; body?: unknown } = {}
): Promise<T> {
	const res = await fetch(path, {
		method: init.method ?? (init.body === undefined ? 'GET' : 'POST'),
		credentials: 'same-origin',
		headers: init.body === undefined ? {} : { 'content-type': 'application/json' },
		body: init.body === undefined ? undefined : JSON.stringify(init.body)
	});
	if (res.status === 204) return undefined as T;
	const text = await res.text();
	const data = text ? (JSON.parse(text) as unknown) : undefined;
	if (!res.ok) {
		throw new ApiFailure(res.status, (data as ApiError | undefined)?.error ?? res.statusText);
	}
	return data as T;
}

/** A failure said plainly for the console. */
export function failureText(e: unknown): string {
	if (e instanceof ApiFailure) {
		switch (e.status) {
			case 401:
				return 'Сесията изтече. Влезте отново.';
			case 403:
				return 'Нямате права за това действие.';
			case 404:
				return 'Не е намерено.';
			case 429:
				return 'Твърде много опити. Опитайте след малко.';
		}
		return e.message;
	}
	return e instanceof Error ? e.message : String(e);
}
