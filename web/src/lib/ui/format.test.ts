import { describe, expect, it } from 'vitest';
import { rate, total } from './format.ts';

// toLocaleString separates thousands with a no-break space.
const plain = (s: string) => s.replace(/\s/g, ' ');

describe('rate', () => {
	it('keeps one decimal below ten', () => {
		expect(rate(5.64)).toBe('5,6');
		expect(plain(rate(12400.4))).toBe('12 400');
	});
});

describe('total', () => {
	it('is exact below a hundred thousand', () => {
		expect(total(0)).toBe('0');
		expect(plain(total(99_999))).toBe('99 999');
	});
	it('is short from a hundred thousand', () => {
		expect(plain(total(1_234_567))).toBe('1,2 млн.');
		expect(plain(total(250_000))).toBe('250 хил.');
	});
});
