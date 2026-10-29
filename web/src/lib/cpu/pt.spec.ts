import { readFile } from 'node:fs/promises';
import { describe, expect, it } from 'vitest';
import vectors from '../../../../templates/pt/vectors.json';
import { agreement, decodeRgb, encodeRgb, gpuAgrees } from '#lib/pt.ts';

// The module the CPU fallback runs; the Vite config builds it before tests start.
const wasmPath = new URL(
	'../../../../target/wasm32-unknown-unknown/release/kernels_wasm.wasm',
	import.meta.url
);

async function load() {
	const { instance } = await WebAssembly.instantiate(await readFile(wasmPath), {});
	const k = instance.exports as unknown as {
		memory: WebAssembly.Memory;
		pt_render_rows(...args: number[]): number;
	};
	return (v: (typeof vectors)[number], rowStart = 0, rowEnd = v.h) => {
		const ptr = k.pt_render_rows(
			v.width,
			v.height,
			v.x,
			v.y,
			v.w,
			v.h,
			v.spp,
			v.pass,
			v.seed,
			rowStart,
			rowEnd
		);
		return Array.from(new Float32Array(k.memory.buffer, ptr, (rowEnd - rowStart) * v.w * 3));
	};
}

describe('WebAssembly path tracer', () => {
	it('matches the shared vectors bit for bit', async () => {
		const render = await load();
		expect(vectors.length).toBeGreaterThan(0);
		for (const v of vectors) {
			expect(render(v), `${v.x},${v.y} ${v.w}×${v.h}`).toEqual(v.rgb);
		}
	});

	it('renders the same pixels whatever rows each thread takes', async () => {
		const render = await load();
		const v = vectors.find((v) => v.h >= 4)!;
		const split = [...render(v, 0, 1), ...render(v, 1, 3), ...render(v, 3, v.h)];
		expect(split).toEqual(v.rgb);
	});
});

describe('rgb encoding', () => {
	it('round-trips little-endian f32', () => {
		const values = new Float32Array([0, 1.5, -2.25, 3.4028234663852886e38, 1e-30]);
		expect(decodeRgb(encodeRgb(values))).toEqual(values);
		// Same bytes as the Rust side: 1.0f32 is 00 00 80 3f.
		expect(encodeRgb(new Float32Array([1]))).toBe(btoa('\x00\x00\x80\x3f'));
	});
});

describe('GPU agreement', () => {
	const expected = Array.from({ length: 100 }, (_, i) => i + 1);

	it('accepts rounding differences and a few diverging samples', () => {
		const actual = expected.map((e, i) => (i < 5 ? e * 1.5 : e * (1 + 1e-5)));
		expect(gpuAgrees(agreement(actual, expected))).toBe(true);
	});

	it('rejects a systematically different render', () => {
		expect(
			gpuAgrees(
				agreement(
					expected.map((e) => e * 1.1),
					expected
				)
			)
		).toBe(false);
		expect(
			gpuAgrees(
				agreement(
					expected.map(() => 0),
					expected
				)
			)
		).toBe(false);
	});
});
