import { describe, expect, it } from 'vitest';
import { deviceDetail, gpuName, platformName, reportedGpu } from './device-name.ts';

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
