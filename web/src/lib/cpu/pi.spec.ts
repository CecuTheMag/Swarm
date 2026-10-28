import { readFile } from 'node:fs/promises';
import { describe, expect, it } from 'vitest';
import vectors from '../../../../templates/pi/vectors.json';

// The module the CPU fallback runs; the Vite config builds it before tests start.
const wasmPath = new URL(
	'../../../../target/wasm32-unknown-unknown/release/kernels_wasm.wasm',
	import.meta.url
);

async function load() {
	const { instance } = await WebAssembly.instantiate(await readFile(wasmPath), {});
	const k = instance.exports as unknown as {
		pi_count_inside_range(seed: number, unitIndex: number, start: number, end: number): number;
	};
	return (seed: number, unitIndex: number, start: number, end: number) =>
		k.pi_count_inside_range(seed, unitIndex, start, end) >>> 0;
}

describe('WebAssembly π kernel', () => {
	it('matches the shared vectors bit for bit', async () => {
		const count = await load();
		expect(vectors.length).toBeGreaterThan(0);
		for (const v of vectors) {
			expect(count(v.seed, v.unit_index, 0, v.samples), JSON.stringify(v)).toBe(v.inside);
		}
	});

	it('counts of split ranges add up to the unit', async () => {
		const count = await load();
		for (const v of vectors.filter((v) => v.samples > 1)) {
			const mid = Math.floor(v.samples / 3);
			const split =
				count(v.seed, v.unit_index, 0, mid) + count(v.seed, v.unit_index, mid, v.samples);
			expect(split).toBe(v.inside);
		}
	});

	it('passes u32 values above 2³¹ unchanged', async () => {
		const count = await load();
		const v = vectors.find((v) => v.seed === 4294967295);
		expect(v).toBeDefined();
		expect(count(v!.seed, v!.unit_index, 0, v!.samples)).toBe(v!.inside);
	});
});
