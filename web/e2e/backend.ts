import type { TestInfo } from '@playwright/test';

// Only the `webgpu` project starts Chromium with WebGPU (SwiftShader). Headless Firefox
// and WebKit on Linux have none, so like Chromium without the flags they must fall
// back to the CPU: that is what an iPhone or a Firefox user without WebGPU gets.
export const expectedBackend = (testInfo: TestInfo) =>
	testInfo.project.name === 'webgpu' ? 'webgpu' : 'wasm';
