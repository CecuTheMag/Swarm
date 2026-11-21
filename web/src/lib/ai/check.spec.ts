import { describe, expect, it } from 'vitest';
import { agree, assertCheck, NEAR_TIE, UnfitError } from './check.ts';
import catalog from '../../../../templates/ai/catalog.json';

const check = {
	prompt: 'List the planets.',
	max_new_tokens: 8,
	tokens: [5, 6, 7, 8, 2],
	margins: [3, 2, 0.1, 4, 5]
};

describe('agree', () => {
	it('takes the same tokens, whatever the margins', () => {
		expect(agree(check, { tokens: [5, 6, 7, 8, 2], margins: [0.1, 9, 3, 1, 1] })).toBe(true);
	});

	it('takes answers that part where both devices chose by a near-tie', () => {
		expect(agree(check, { tokens: [5, 6, 9, 1, 2], margins: [3, 2, 0.3, 1, 1] })).toBe(true);
	});

	it('refuses answers that part where either device was sure', () => {
		expect(agree(check, { tokens: [5, 6, 9, 1, 2], margins: [3, 2, NEAR_TIE, 1, 1] })).toBe(false);
		expect(agree(check, { tokens: [5, 6, 7, 9, 2], margins: [3, 2, 0.1, 0.1, 1] })).toBe(false);
	});

	it('never takes a different first token', () => {
		const a = { tokens: [1, 2], margins: [0.1, 1] };
		expect(agree(a, { tokens: [3, 2], margins: [0.1, 1] })).toBe(false);
	});

	it('refuses an answer cut short', () => {
		expect(agree(check, { tokens: [5, 6, 7], margins: [3, 2, 0.1] })).toBe(false);
	});
});

describe('assertCheck', () => {
	it('passes the known answer', () => {
		expect(() => assertCheck('M', check, check)).not.toThrow();
	});

	it('names where a wrong answer parts', () => {
		const wrong = { tokens: [5, 6, 7, 9, 9], margins: [3, 2, 0.1, 4, 5] };
		expect(() => assertCheck('M', check, wrong)).toThrow(UnfitError);
		expect(() => assertCheck('M', check, wrong)).toThrow(/M wrongly.*token 4/);
	});

	it('every catalog model agrees with its own known answer', () => {
		for (const m of catalog.models) {
			expect(m.check.tokens.length).toBe(m.check.margins.length);
			expect(() => assertCheck(m.name, m.check, m.check)).not.toThrow();
		}
	});
});
