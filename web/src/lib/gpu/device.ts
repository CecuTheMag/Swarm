import type { DeviceInfo } from '#lib/protocol/DeviceInfo.ts';
import { reportedGpu } from '#lib/device-name.ts';

export interface Gpu {
	device: GPUDevice;
	info: DeviceInfo;
}

export class GpuUnavailableError extends Error {}

/** Acquires a WebGPU device, preferring the discrete GPU when there are several. */
export async function requestGpu(): Promise<Gpu> {
	if (!('gpu' in navigator) || !navigator.gpu) {
		throw new GpuUnavailableError('WebGPU is not supported by this browser');
	}
	const adapter = await navigator.gpu.requestAdapter({ powerPreference: 'high-performance' });
	if (!adapter) {
		throw new GpuUnavailableError('No WebGPU adapter is available on this device');
	}
	const device = await adapter.requestDevice();
	return {
		device,
		info: {
			backend: 'webgpu',
			// Blank when the browser hides the card (Brave's fingerprinting protection): the
			// system then stands in, so a phone and a laptop still differ in the console.
			...reportedGpu(adapter.info, navigator.userAgent),
			// Language-model units go only to devices whose GPU can hold the model (spec 0017).
			max_buffer_size: adapter.limits.maxBufferSize
		}
	};
}
