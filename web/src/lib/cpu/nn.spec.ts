import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { describe, expect, it } from 'vitest';
import vectors from '../../../../templates/nn/vectors.json';
import { MNIST_SHA256, nnWeightWords } from '#lib/nn.ts';

// The module the CPU fallback runs; the Vite config builds it before tests start.
const wasmPath = new URL(
	'../../../../target/wasm32-unknown-unknown/release/kernels_wasm.wasm',
	import.meta.url
);
const mnistPath = new URL('../../../../templates/nn/mnist14.bin', import.meta.url);

interface Exports {
	memory: WebAssembly.Memory;
	nn_mnist_buffer(): number;
	nn_mnist_load(): number;
	nn_train(...args: number[]): number;
}

describe('WebAssembly network kernel', () => {
	it('the packed MNIST is the pinned file', async () => {
		const hash = createHash('sha256')
			.update(await readFile(mnistPath))
			.digest('hex');
		expect(hash).toBe(MNIST_SHA256);
	});

	it('trains on MNIST handed over by the page as the Rust reference does', async () => {
		const { instance } = await WebAssembly.instantiate(await readFile(wasmPath), {});
		const k = instance.exports as unknown as Exports;
		// Not loaded yet: training refuses.
		expect(k.nn_train(1, 8, 1, 6554, 3, 2, 1)).toBe(0);

		const bytes = await readFile(mnistPath);
		const ptr = k.nn_mnist_buffer();
		new Uint8Array(k.memory.buffer, ptr, bytes.length).set(bytes);
		expect(k.nn_mnist_load()).toBe(1);

		const mnist = vectors.filter((v) => v.dataset === 'mnist');
		expect(mnist.length).toBeGreaterThan(0);
		for (const v of mnist) {
			const out = k.nn_train(1, v.hidden, v.layers, v.lr_q16, v.batch_log2, v.steps, v.seed);
			const words = new Uint32Array(k.memory.buffer, out, 17 + nnWeightWords(v, 196)).slice();
			expect(words[0], JSON.stringify(v)).toBe(v.correct);
			expect(Array.from(words.subarray(1, 17))).toEqual(v.curve);
			const weights = Buffer.from(words.buffer, 17 * 4);
			expect(createHash('sha256').update(weights).digest('hex')).toBe(v.digest);
		}
	});
});
