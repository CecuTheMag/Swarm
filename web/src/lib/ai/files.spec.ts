import { describe, expect, it, vi } from 'vitest';
import { catalogFile, ModelFileError, sha256OfBytes, verifiedFetch } from './files.ts';
import type { AiCatalog } from '#lib/protocol/AiCatalog.ts';

const ORIGIN = 'https://swarm.test';
const bytes = (text: string) => new TextEncoder().encode(text).buffer as ArrayBuffer;

async function catalogWith(content: string): Promise<AiCatalog> {
	return {
		version: 1,
		models: [
			{
				id: 'org/model',
				name: 'Model',
				licence: 'Apache-2.0',
				source: 'https://example.test/org/model',
				revision: 'a'.repeat(40),
				dtype: 'q4f16',
				chat: true,
				test: false,
				min_gpu_buffer: null,
				files: [
					{
						path: 'onnx/model.onnx',
						size: bytes(content).byteLength,
						sha256: await sha256OfBytes(bytes(content))
					}
				],
				check: { prompt: 'Hi', max_new_tokens: 1, tokens: [1], margins: [1] }
			}
		]
	};
}

/** Cache Storage in memory. */
function memoryCaches() {
	const entries = new Map<string, ArrayBuffer>();
	const cache = {
		match: async (key: string) => {
			const b = entries.get(key);
			return b ? new Response(b.slice(0)) : undefined;
		},
		put: async (key: string, res: Response) => void entries.set(key, await res.arrayBuffer()),
		delete: async (key: string) => entries.delete(key)
	};
	return { entries, caches: { open: async () => cache } as unknown as CacheStorage };
}

const URL_OK = `${ORIGIN}/api/models/org/model/onnx/model.onnx`;

describe('model files', () => {
	it('finds only files the catalog lists', async () => {
		const catalog = await catalogWith('weights');
		expect(catalogFile(catalog, '/api/models/org/model/onnx/model.onnx')?.file.path).toBe(
			'onnx/model.onnx'
		);
		expect(catalogFile(catalog, '/api/models/org/model/onnx/other.onnx')).toBeUndefined();
		expect(catalogFile(catalog, '/api/models/org/modelx/onnx/model.onnx')).toBeUndefined();
		expect(catalogFile(catalog, '/elsewhere/org/model/onnx/model.onnx')).toBeUndefined();
	});

	it('checks a download and reads the second load from the cache', async () => {
		const catalog = await catalogWith('weights');
		const { caches } = memoryCaches();
		const network = vi.fn(async () => new Response(bytes('weights')));
		const seen: string[] = [];
		const f = verifiedFetch(catalog, {
			fetch: network as unknown as typeof fetch,
			caches,
			origin: ORIGIN,
			onFile: (_, from) => seen.push(from)
		});
		expect(await (await f(URL_OK)).text()).toBe('weights');
		expect(await (await f(URL_OK)).text()).toBe('weights');
		expect(network).toHaveBeenCalledTimes(1);
		expect(seen).toEqual(['network', 'cache']);
	});

	it('refuses a file whose hash differs and keeps no copy of it', async () => {
		const catalog = await catalogWith('weights');
		const { caches, entries } = memoryCaches();
		const f = verifiedFetch(catalog, {
			fetch: (async () => new Response(bytes('weightz'))) as unknown as typeof fetch,
			caches,
			origin: ORIGIN
		});
		await expect(f(URL_OK)).rejects.toThrow(/SHA-256 differs/);
		expect(entries.size).toBe(0);
	});

	it('replaces a damaged cached copy', async () => {
		const catalog = await catalogWith('weights');
		const { caches, entries } = memoryCaches();
		entries.set(URL_OK, bytes('tampered'));
		const network = vi.fn(async () => new Response(bytes('weights')));
		const f = verifiedFetch(catalog, {
			fetch: network as unknown as typeof fetch,
			caches,
			origin: ORIGIN
		});
		expect(await (await f(URL_OK)).text()).toBe('weights');
		expect(network).toHaveBeenCalledTimes(1);
		expect(new TextDecoder().decode(entries.get(URL_OK))).toBe('weights');
	});

	it('refuses other origins and files outside the catalog', async () => {
		const catalog = await catalogWith('weights');
		const network = vi.fn(async () => new Response('x'));
		const f = verifiedFetch(catalog, { fetch: network as unknown as typeof fetch, origin: ORIGIN });
		await expect(f('https://huggingface.co/org/model/resolve/main/x')).rejects.toThrow(
			ModelFileError
		);
		await expect(f(`${ORIGIN}/api/models/org/model/secret.bin`)).rejects.toThrow(/not in the/);
		expect(network).not.toHaveBeenCalled();
		// The runtime's own files on this site pass through.
		await f(`${ORIGIN}/_app/immutable/ort.wasm`);
		expect(network).toHaveBeenCalledTimes(1);
	});
});
