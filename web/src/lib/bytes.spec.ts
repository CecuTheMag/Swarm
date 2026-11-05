import { describe, expect, it } from 'vitest';
import { fromBase64, fromHex, sha256OfBytes, toBase64, toHex } from './bytes.ts';

describe('bytes', () => {
	it('round-trips base64, also above one chunk', () => {
		for (const length of [0, 1, 2, 3, 0x8000, 0x8001, 100_000]) {
			const bytes = Uint8Array.from({ length }, (_, i) => (i * 31 + 7) & 0xff);
			expect(fromBase64(toBase64(bytes))).toEqual(bytes);
		}
		expect(toBase64(new Uint8Array([1, 2, 3]))).toBe('AQID');
	});

	it('round-trips hex and refuses what is not hex', () => {
		expect(toHex(new Uint8Array([0, 15, 255]))).toBe('000fff');
		expect(fromHex('00 0f FF')).toEqual(new Uint8Array([0, 15, 255]));
		expect(fromHex('')).toEqual(new Uint8Array());
		for (const bad of ['0', 'zz', '0x00']) expect(() => fromHex(bad)).toThrow();
	});

	it('hashes like the coordinator', async () => {
		expect(await sha256OfBytes(new TextEncoder().encode('abc'))).toBe(
			'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad'
		);
	});
});
