import { describe, expect, it } from 'vitest';
import { rate } from './format.ts';

describe('rate', () => {
	it('writes a decimal comma below ten and whole numbers above', () => {
		expect(rate(5.64)).toBe('5,6');
		expect(rate(12.4)).toBe('12');
		expect(rate(0.1)).toBe('0,1');
	});

	it('groups thousands with a space', () => {
		expect(rate(12400).replace(/\s/g, ' ')).toBe('12 400');
	});
});
