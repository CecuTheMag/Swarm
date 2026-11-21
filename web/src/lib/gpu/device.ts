import type { DeviceInfo } from '#lib/protocol/DeviceInfo.ts';

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
			vendor: adapter.info.vendor || 'unknown',
			architecture: adapter.info.architecture || adapter.info.description || 'unknown',
			// Language-model units go only to devices whose GPU can hold the model (spec 0017).
			max_buffer_size: adapter.limits.maxBufferSize
		}
	};
}
