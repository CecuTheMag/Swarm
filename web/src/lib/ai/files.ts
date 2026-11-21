// Model files for language-model units (spec 0017). A model is data, but it decides what
// a device computes, so it is pinned like a kernel: only files the catalog lists are
// loaded, each is checked against the catalog's SHA-256 before use, and checked copies are
// kept in Cache Storage so a device downloads a model once.

import type { AiCatalog } from '#lib/protocol/AiCatalog.ts';
import type { AiModel } from '#lib/protocol/AiModel.ts';
import type { AiModelFile } from '#lib/protocol/AiModelFile.ts';

/** Where the coordinator serves model files: `/api/models/{model id}/{file}`. */
export const MODELS_PATH = '/api/models/';
export const CACHE_NAME = 'swarm-models-v1';

export class ModelFileError extends Error {}

export interface CatalogFile {
	model: AiModel;
	file: AiModelFile;
}

/** The catalog entry a URL path names, if any. */
export function catalogFile(catalog: AiCatalog, pathname: string): CatalogFile | undefined {
	if (!pathname.startsWith(MODELS_PATH)) return undefined;
	const rest = decodeURIComponent(pathname.slice(MODELS_PATH.length));
	for (const model of catalog.models) {
		if (!rest.startsWith(`${model.id}/`)) continue;
		const file = model.files.find((f) => f.path === rest.slice(model.id.length + 1));
		if (file) return { model, file };
	}
	return undefined;
}

export async function sha256OfBytes(bytes: ArrayBuffer): Promise<string> {
	const digest = await crypto.subtle.digest('SHA-256', bytes);
	return Array.from(new Uint8Array(digest), (b) => b.toString(16).padStart(2, '0')).join('');
}

export interface FetchDeps {
	fetch: typeof fetch;
	/** Cache Storage, where the browser has it (not on plain HTTP outside localhost). */
	caches?: CacheStorage;
	/** The page's origin; model files come only from it. */
	origin: string;
	/** Told whether each model file came from the cache or the network. */
	onFile?: (path: string, from: 'cache' | 'network', bytes: number) => void;
}

function respond(bytes: ArrayBuffer, path: string): Response {
	return new Response(bytes, {
		status: 200,
		headers: {
			'content-type': path.endsWith('.json') ? 'application/json' : 'application/octet-stream',
			'content-length': String(bytes.byteLength)
		}
	});
}

/**
 * A `fetch` for the model runtime: model files only from this origin and the catalog,
 * each checked against its SHA-256, from the cache when a checked copy is there. Anything
 * else from another origin is refused, so the runtime cannot reach other sites.
 */
export function verifiedFetch(catalog: AiCatalog, deps: FetchDeps): typeof fetch {
	let cache: Promise<Cache | undefined> | undefined;
	const openCache = () =>
		(cache ??= deps.caches
			? deps.caches.open(CACHE_NAME).catch(() => undefined)
			: Promise.resolve(undefined));

	return async (input, init) => {
		const href = typeof input === 'string' ? input : input instanceof URL ? input.href : input.url;
		const url = new URL(href, deps.origin);
		if (url.origin !== deps.origin) {
			throw new ModelFileError(`refusing to load ${url.origin}: models come from this site only`);
		}
		if (!url.pathname.startsWith(MODELS_PATH)) return deps.fetch(input, init);
		const entry = catalogFile(catalog, url.pathname);
		if (!entry) throw new ModelFileError(`${url.pathname} is not in the model catalog`);
		const { file } = entry;
		const key = url.origin + url.pathname;
		const store = await openCache();

		const cached = await store?.match(key);
		if (cached) {
			const bytes = await cached.arrayBuffer();
			if (bytes.byteLength === file.size && (await sha256OfBytes(bytes)) === file.sha256) {
				deps.onFile?.(file.path, 'cache', bytes.byteLength);
				return respond(bytes, file.path);
			}
			// A damaged copy: fetched again below.
			await store?.delete(key);
		}

		const res = await deps.fetch(key);
		if (!res.ok) throw new ModelFileError(`${url.pathname}: HTTP ${res.status}`);
		const bytes = await res.arrayBuffer();
		if (bytes.byteLength !== file.size || (await sha256OfBytes(bytes)) !== file.sha256) {
			throw new ModelFileError(
				`${url.pathname}: SHA-256 differs from the model catalog; refusing to use it`
			);
		}
		deps.onFile?.(file.path, 'network', bytes.byteLength);
		// Not keeping a copy (storage full, private mode) only costs a download next time.
		await store?.put(key, respond(bytes, file.path)).catch(() => {});
		return respond(bytes, file.path);
	};
}
