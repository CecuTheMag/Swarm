import type { TestInfo } from '@playwright/test';

// Only the `webgpu` project starts Chromium with WebGPU (SwiftShader). Headless Firefox
// and WebKit on Linux have none, so like Chromium without the flags they must fall
// back to the CPU: that is what an iPhone or a Firefox user without WebGPU gets.
// WebKit on macOS is Safari's engine on Metal and has WebGPU (checked 2026-11-11 on an
// M1 Max), so there it must take the GPU path.
export const expectedBackend = (testInfo: TestInfo) => {
	const project = testInfo.project.name;
	if (project === 'webgpu') return 'webgpu';
	if (project === 'webkit' && process.platform === 'darwin') return 'webgpu';
	return 'wasm';
};
