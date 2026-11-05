import { readFile } from 'node:fs/promises';
import { describe, expect, it } from 'vitest';
import vectors from '../../../../templates/tt/vectors.json';
import { sha256OfBytes } from '#lib/bytes.ts';
import { solveTimetable, type TtExports } from '#lib/tt.ts';

// The module the CPU threads run; the Vite config builds it before tests start.
const wasmPath = new URL(
	'../../../../target/wasm32-unknown-unknown/release/kernels_wasm.wasm',
	import.meta.url
);

describe('WebAssembly timetable solver', () => {
	it('gives the shared vectors exactly', async () => {
		const { instance } = await WebAssembly.instantiate(await readFile(wasmPath), {});
		const k = instance.exports as unknown as TtExports;
		expect(vectors.length).toBeGreaterThan(0);
		for (const v of vectors) {
			const school = new TextEncoder().encode(JSON.stringify(v.school));
			const found = solveTimetable(k, school, v.seed, v.index, v.iterations);
			expect(found.cost).toBe(v.cost);
			expect(await sha256OfBytes(found.slots)).toBe(v.slots_sha256);
		}
	});

	it('refuses a school that is not valid', async () => {
		const { instance } = await WebAssembly.instantiate(await readFile(wasmPath), {});
		const k = instance.exports as unknown as TtExports;
		const school = new TextEncoder().encode('{"name": "x"}');
		expect(() => solveTimetable(k, school, 1, 0, 10_000)).toThrow(/not valid/);
	});
});
