import { describe, expect, it } from 'vitest';
import { pauseReason, restMs } from './pacing.ts';

describe('restMs', () => {
	it('rests so the device computes the chosen share of the time', () => {
		expect(restMs(300, 100)).toBe(0);
		expect(restMs(300, 75)).toBe(100);
		expect(restMs(300, 50)).toBe(300);
		expect(restMs(300, 25)).toBe(900);
		for (const power of [25, 50, 75]) {
			const busy = 300 / (300 + restMs(300, power));
			expect(busy).toBeCloseTo(power / 100);
		}
	});
});

describe('pauseReason', () => {
	it('pauses for a hidden page first, then for a low battery that is not charging', () => {
		expect(pauseReason(false, null)).toBeNull();
		expect(pauseReason(true, null)).toBe('hidden');
		expect(pauseReason(true, { level: 0.1, charging: false })).toBe('hidden');
		expect(pauseReason(false, { level: 0.29, charging: false })).toBe('battery');
		expect(pauseReason(false, { level: 0.29, charging: true })).toBeNull();
		expect(pauseReason(false, { level: 0.3, charging: false })).toBeNull();
	});
});
