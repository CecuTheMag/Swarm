import { defineConfig } from '@playwright/test';

// The browser tests drive the real system: coordinator + built site + WebGPU.
// They use their own database so dev data never interferes.
const DATABASE_URL =
	process.env.E2E_DATABASE_URL ?? 'postgres://swarm:swarm@localhost:7732/swarm_e2e';

export default defineConfig({
	testDir: 'e2e',
	testMatch: '**/*.e2e.ts',
	timeout: 60_000,
	use: {
		baseURL: 'http://localhost:7702',
		launchOptions: {
			// Headless Chromium exposes WebGPU (software adapter) only with these flags.
			args: ['--enable-unsafe-webgpu', '--enable-features=Vulkan']
		}
	},
	webServer: [
		{
			command: 'cargo run -p coordinator',
			cwd: '..',
			url: 'http://127.0.0.1:7700/healthz',
			env: { DATABASE_URL, COORDINATOR_ADDR: '127.0.0.1:7700', RUST_LOG: 'warn' },
			reuseExistingServer: false,
			timeout: 300_000
		},
		{
			command: 'pnpm build && pnpm preview',
			url: 'http://localhost:7702',
			reuseExistingServer: false,
			timeout: 120_000
		}
	]
});
