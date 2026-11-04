import { readFile } from 'node:fs/promises';
import { describe, expect, it } from 'vitest';
import { networkOf, rasterize, SIDE, type Model } from './digit.ts';

// The module the CPU fallback runs; the Vite config builds it before tests start.
const wasmPath = new URL(
	'../../../../target/wasm32-unknown-unknown/release/kernels_wasm.wasm',
	import.meta.url
);

const DIM = SIDE * SIDE;
const TRAIN = 4096;
const VALIDATION = 1024;

interface Exports {
	memory: WebAssembly.Memory;
	nn_glyphs_pixels(): number;
	nn_glyphs_labels(): number;
	nn_train(...args: number[]): number;
}

describe('trying out a trained network', () => {
	it('reads the validation digits exactly as the devices scored the network', async () => {
		const { instance } = await WebAssembly.instantiate(await readFile(wasmPath), {});
		const k = instance.exports as unknown as Exports;
		const [hidden, layers] = [32, 2];
		const ptr = k.nn_train(0, hidden, layers, 13107, 5, 200, 7);
		const sizes = [DIM, ...Array<number>(layers).fill(hidden), 10];
		let words = 0;
		for (let l = 0; l + 1 < sizes.length; l++) words += sizes[l] * sizes[l + 1] + sizes[l + 1];
		const out = new Int32Array(k.memory.buffer, ptr, 17 + words).slice();
		const pixelsPtr = k.nn_glyphs_pixels();
		const labelsPtr = k.nn_glyphs_labels();
		const pixels = new Uint8Array(k.memory.buffer, pixelsPtr, (TRAIN + VALIDATION) * DIM).slice();
		const labels = new Uint8Array(k.memory.buffer, labelsPtr, TRAIN + VALIDATION).slice();

		// The model as the coordinator's download gives it: real numbers, Q24 / 2²⁴.
		const network = [];
		let at = 17;
		for (let l = 0; l + 1 < sizes.length; l++) {
			const n = sizes[l] * sizes[l + 1];
			const real = (v: number) => v / 2 ** 24;
			network.push({
				inputs: sizes[l],
				outputs: sizes[l + 1],
				activation: l + 2 < sizes.length ? 'relu' : 'softmax',
				weights: Array.from(out.subarray(at, at + n), real),
				biases: Array.from(out.subarray(at + n, at + n + sizes[l + 1]), real)
			});
			at += n + sizes[l + 1];
		}
		const model: Model = {
			hidden,
			layers,
			learning_rate: 0.2,
			batch: 32,
			accuracy: out[0] / VALIDATION,
			digest: '',
			network
		};

		const read = networkOf(model);
		let correct = 0;
		for (let i = 0; i < VALIDATION; i++) {
			const image = pixels.subarray((TRAIN + i) * DIM, (TRAIN + i + 1) * DIM);
			const reading = read(image);
			expect(reading.probabilities.reduce((s, p) => s + p, 0)).toBeCloseTo(1, 9);
			if (reading.digit === labels[TRAIN + i]) correct++;
		}
		expect(correct).toBeGreaterThan(VALIDATION / 2);
		expect(correct).toBe(out[0]);
	});

	it('draws strokes into the dataset box, centred, with its pen', () => {
		expect(rasterize([]).every((v) => v === 0)).toBe(true);
		// A vertical stroke far off-centre, at any scale: centred, margins blank.
		const one = rasterize([
			[
				{ x: 900, y: 100 },
				{ x: 900, y: 700 }
			]
		]);
		const ink = (x: number, y: number) => one[y * SIDE + x];
		expect(ink(7, 7)).toBe(255);
		expect(ink(6, 7)).toBe(255);
		expect(ink(3, 7)).toBe(0);
		expect(ink(7, 0)).toBeLessThan(255);
		for (let y = 0; y < SIDE; y++) {
			for (let x = 0; x < SIDE; x++) {
				expect(ink(x, y)).toBe(ink(SIDE - 1 - x, y));
				expect(ink(x, y)).toBe(ink(x, SIDE - 1 - y));
			}
		}
	});
});
