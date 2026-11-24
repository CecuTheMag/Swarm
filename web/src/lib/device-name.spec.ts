import { describe, expect, it } from 'vitest';
import {
	announced,
	deviceDetail,
	gpuName,
	isHandheld,
	platformName,
	reportedGpu
} from './device-name.ts';

const PIXEL =
	'Mozilla/5.0 (Linux; Android 16; Pixel 9) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/157.0.0.0 Mobile Safari/537.36';
const MAC =
	'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/157.0.0.0 Safari/537.36';

describe('platformName', () => {
	it('tells a phone from a laptop', () => {
		expect(platformName(PIXEL)).toBe('Android');
		expect(platformName(MAC)).toBe('macOS');
		expect(platformName('Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X)')).toBe('iOS');
		expect(platformName('Mozilla/5.0 (Windows NT 10.0; Win64; x64)')).toBe('Windows');
		expect(platformName('Mozilla/5.0 (X11; CrOS x86_64 16000.0.0)')).toBe('ChromeOS');
		expect(platformName('Mozilla/5.0 (X11; Linux x86_64)')).toBe('Linux');
		expect(platformName('curl/8')).toBeUndefined();
	});
});

describe('reportedGpu', () => {
	it('passes on what the adapter reports', () => {
		expect(reportedGpu({ vendor: 'apple', architecture: 'metal-3' }, MAC)).toEqual({
			vendor: 'apple',
			architecture: 'metal-3'
		});
	});

	it('falls back to the description when there is no architecture', () => {
		expect(
			reportedGpu({ vendor: 'arm', architecture: '', description: 'Mali-G715' }, PIXEL)
		).toEqual({ vendor: 'arm', architecture: 'Mali-G715' });
	});

	it('names the system when the browser hides the card', () => {
		for (const info of [{}, { vendor: '', architecture: '', description: '' }]) {
			expect(reportedGpu(info, PIXEL)).toEqual({
				vendor: 'unknown',
				architecture: 'unknown (Android)'
			});
			expect(reportedGpu(info, MAC).architecture).toBe('unknown (macOS)');
		}
		expect(reportedGpu({}, 'curl/8')).toEqual({ vendor: 'unknown', architecture: 'unknown' });
	});

	it('keeps a vendor that is known when only the architecture is hidden', () => {
		expect(reportedGpu({ vendor: 'qualcomm' }, PIXEL)).toEqual({
			vendor: 'qualcomm',
			architecture: 'unknown'
		});
	});
});

describe('gpuName', () => {
	it('reads a known card', () => {
		expect(gpuName('apple', 'metal-3')).toBe('apple · metal-3');
		expect(gpuName('qualcomm', 'unknown')).toBe('qualcomm');
		expect(gpuName('unknown', 'valhall')).toBe('valhall');
	});

	it('says the browser hides a hidden card, with the system when known', () => {
		expect(gpuName('unknown', 'unknown')).toBe('скрита от браузъра');
		expect(gpuName('', '')).toBe('скрита от браузъра');
		expect(gpuName('unknown', 'unknown (Android)')).toBe('скрита от браузъра · Android');
	});
});

describe('deviceDetail', () => {
	it('names a card by gpuName and a CPU by its threads', () => {
		expect(
			deviceDetail({ backend: 'webgpu', vendor: 'unknown', architecture: 'unknown (macOS)' })
		).toBe('скрита от браузъра · macOS');
		expect(deviceDetail({ backend: 'wasm', vendor: 'cpu', architecture: '4 threads' })).toBe(
			'cpu · 4 threads'
		);
	});
});

describe('isHandheld', () => {
	const IPHONE =
		'Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1';

	it('is true for phones and tablets', () => {
		expect(isHandheld({ userAgent: PIXEL })).toBe(true);
		expect(isHandheld({ userAgent: IPHONE })).toBe(true);
		expect(isHandheld({ userAgent: 'Mozilla/5.0 (iPad; CPU OS 18_0 like Mac OS X)' })).toBe(true);
		expect(isHandheld({ userAgent: MAC, maxTouchPoints: 5 })).toBe(true); // iPad as a Mac
		expect(isHandheld({ userAgent: 'x', userAgentData: { mobile: true } })).toBe(true);
	});

	it('is false for computers, Brave and Chrome on a Mac included', () => {
		expect(isHandheld({ userAgent: MAC })).toBe(false);
		expect(isHandheld({ userAgent: MAC, maxTouchPoints: 0 })).toBe(false);
		expect(isHandheld({ userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64)' })).toBe(false);
		expect(isHandheld({ userAgent: 'Mozilla/5.0 (X11; Linux x86_64)', maxTouchPoints: 10 })).toBe(
			false
		);
		expect(isHandheld({ userAgent: MAC, userAgentData: { mobile: false } })).toBe(false);
	});
});

describe('announced', () => {
	const gpu = {
		backend: 'webgpu',
		vendor: 'apple',
		architecture: 'metal-3',
		max_buffer_size: 4e9
	} as const;

	it('keeps the buffer size of a computer, so it can take a language model', () => {
		expect(announced(gpu, { userAgent: MAC })).toEqual(gpu);
	});

	it('leaves it out for a phone, so none is given', () => {
		const phone = announced(gpu, { userAgent: PIXEL });
		expect(phone).toEqual({ backend: 'webgpu', vendor: 'apple', architecture: 'metal-3' });
		expect('max_buffer_size' in phone).toBe(false);
	});
});
