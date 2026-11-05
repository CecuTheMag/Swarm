import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { runUnit } from './abi.ts';

// Built by the Rust fixtures (crates/programs/src/fixtures.rs).
const fixture = (name: string) =>
	WebAssembly.compile(readFileSync(new URL(`./fixtures/${name}.wasm`, import.meta.url)));

/** FNV-1a (64-bit), as the fixture computes it. */
function fnv(input: Uint8Array): Uint8Array {
	let h = 0xcbf29ce484222325n;
	for (const b of input) h = BigInt.asUintN(64, (h ^ BigInt(b)) * 0x100000001b3n);
	const out = new Uint8Array(8);
	new DataView(out.buffer).setBigUint64(0, h, true);
	return out;
}

describe('runUnit', () => {
	it('runs a program on its input', async () => {
		const module = await fixture('fnv');
		for (const input of [
			new Uint8Array(),
			new TextEncoder().encode('swarm'),
			new Uint8Array(5000).fill(7)
		]) {
			expect(await runUnit(module, input)).toEqual(fnv(input));
		}
	});

	it('starts every unit from a fresh instance', async () => {
		const module = await fixture('fnv');
		const a = await runUnit(module, new Uint8Array([1]));
		await runUnit(module, new Uint8Array([2, 3]));
		expect(await runUnit(module, new Uint8Array([1]))).toEqual(a);
	});

	it('reports a failing program', async () => {
		await expect(runUnit(await fixture('trap'), new Uint8Array())).rejects.toThrow(/unreachable/);
	});

	it('refuses a module with imports', async () => {
		// (module (import "env" "f" (func)))
		const bytes = new Uint8Array([
			0, 97, 115, 109, 1, 0, 0, 0, 1, 4, 1, 96, 0, 0, 2, 9, 1, 3, 101, 110, 118, 1, 102, 0, 0
		]);
		const module = await WebAssembly.compile(bytes);
		await expect(runUnit(module, new Uint8Array())).rejects.toThrow();
	});
});
