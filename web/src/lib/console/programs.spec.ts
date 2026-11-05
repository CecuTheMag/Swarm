import { describe, expect, it } from 'vitest';
import { fromBase64 } from '#lib/bytes.ts';
import { encodeArgs, parseInputs } from './programs.ts';

describe('program inputs', () => {
	const text = (b64: string) => new TextDecoder().decode(fromBase64(b64));

	it('reads one unit per line', () => {
		expect(parseInputs('първи\nвтори\r\n\nпоследен\n').map(text)).toEqual([
			'първи',
			'втори',
			'',
			'последен'
		]);
	});

	it('reads a JSON array of base64', () => {
		expect(parseInputs(' ["AQI=", ""] ')).toEqual(['AQI=', '']);
		expect(() => parseInputs('[1, 2]')).toThrow();
		expect(() => parseInputs('[not json')).toThrow();
	});

	it('encodes arguments as text or hex', () => {
		expect(fromBase64(encodeArgs('ab', false))).toEqual(new Uint8Array([97, 98]));
		expect(fromBase64(encodeArgs('40 42 0f 00', true))).toEqual(new Uint8Array([64, 66, 15, 0]));
		expect(encodeArgs('', true)).toBe('');
		for (const bad of ['4', 'zz']) expect(() => encodeArgs(bad, true)).toThrow();
	});
});
