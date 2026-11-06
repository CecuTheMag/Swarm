import { defineConfig } from '@playwright/test';

// The browser tests drive the real system: coordinator + built site + WebGPU.
// They use their own database so dev data never interferes.
const DATABASE_URL =
	process.env.E2E_DATABASE_URL ?? 'postgres://swarm:swarm@localhost:7732/swarm_e2e';

export default defineConfig({
	testDir: 'e2e',
	testMatch: '**/*.e2e.ts',
	timeout: 60_000,
	// One coordinator serves every test, and the worker test counts connected workers.
	workers: 1,
	// A failure on CI keeps its trace (CI uploads test-results).
	use: { baseURL: 'http://localhost:7702', trace: 'retain-on-failure' },
	projects: [
		{
			name: 'webgpu',
			use: {
				launchOptions: {
					// Headless Chromium exposes WebGPU (software adapter) only with these flags.
					args: ['--enable-unsafe-webgpu', '--enable-features=Vulkan']
				}
			}
		},
		// Without the flags there is no WebGPU, so workers fall back to the CPU.
		{ name: 'cpu' },
		{ name: 'firefox', use: { browserName: 'firefox' } },
		// Safari's engine. Needs system libraries Playwright supports on Ubuntu only:
		// `just e2e-webkit` runs it in Playwright's container.
		{ name: 'webkit', use: { browserName: 'webkit' } }
	],
	webServer: [
		{
			command: 'cargo run -p coordinator',
			cwd: '..',
			url: 'http://127.0.0.1:7700/healthz',
			// One browser worker per test: it cannot verify itself (spec 0004).
			env: {
				DATABASE_URL,
				COORDINATOR_ADDR: '127.0.0.1:7700',
				RUST_LOG: 'warn',
				SWARM_REDUNDANCY: '1',
				// As on the public server; the same password as e2e/submit.ts.
				SWARM_SUBMIT_TOKEN: 'e2e-submit-password',
				// The platform admin the console tests sign in as (e2e/submit.ts).
				SWARM_BOOTSTRAP_ADMIN: 'e2e-admin@swarm.test:e2e admin password'
			},
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
