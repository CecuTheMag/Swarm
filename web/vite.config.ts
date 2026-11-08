import { execFileSync } from 'node:child_process';
import { defineConfig } from 'vitest/config';
import { searchForWorkspaceRoot, type Plugin } from 'vite';
import adapter from '@sveltejs/adapter-node';
import { sveltekit } from '@sveltejs/kit/vite';

/** Where cargo writes the CPU-fallback kernels that `#lib/templates.ts` imports. */
const KERNELS_WASM_DIR = '../target/wasm32-unknown-unknown/release';

/** Builds crates/kernels-wasm before anything imports it, so the site never ships a stale module. */
function kernelsWasm(): Plugin {
	let built = false;
	return {
		name: 'swarm:kernels-wasm',
		buildStart() {
			if (built) return;
			execFileSync(
				'cargo',
				['build', '-p', 'kernels-wasm', '--target', 'wasm32-unknown-unknown', '--release'],
				{ cwd: '..', stdio: 'inherit' }
			);
			built = true;
		}
	};
}

export default defineConfig({
	plugins: [
		kernelsWasm(),
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},
			adapter: adapter(),
			// The site loads nothing from other origins and no other site may frame it
			// (spec 0016). Inline scripts carry a nonce per response; WebAssembly compiles
			// from the site's own modules; styles may be inline, as Svelte sets style
			// attributes. Older Safari does not count a same-origin WebSocket as 'self',
			// hence ws: and wss: for the worker's connection.
			csp: {
				mode: 'auto',
				directives: {
					'default-src': ['self'],
					'script-src': ['self', 'wasm-unsafe-eval'],
					'style-src': ['self', 'unsafe-inline'],
					'img-src': ['self', 'data:', 'blob:'],
					'font-src': ['self', 'data:'],
					'connect-src': ['self', 'ws:', 'wss:'],
					'worker-src': ['self'],
					'object-src': ['none'],
					'base-uri': ['self'],
					'form-action': ['self'],
					'frame-ancestors': ['none']
				}
			}
		})
	],
	server: {
		host: true,
		port: 7701,
		strictPort: true,
		// `just share` serves the dev server over HTTPS at this machine's
		// Tailscale name, so lab PCs and phones get a secure context.
		allowedHosts: ['.ts.net'],
		fs: { allow: [searchForWorkspaceRoot(process.cwd()), '../templates', KERNELS_WASM_DIR] },
		// Same origin for the page and the coordinator, so WebGPU's secure-context
		// requirement is met by a single TLS endpoint.
		proxy: {
			// The Host header stays the page's, as behind Caddy in production: the coordinator
			// checks that changes come from this site (Origin = Host, spec 0010).
			'/api': { target: 'http://localhost:7700', changeOrigin: false },
			'/ws': { target: 'ws://localhost:7700', ws: true }
		}
	},
	preview: {
		port: 7702,
		strictPort: true
	},
	test: {
		expect: { requireAssertions: true },
		projects: [
			{
				extends: './vite.config.ts',
				test: {
					name: 'server',
					environment: 'node',
					include: ['src/**/*.{test,spec}.{js,ts}'],
					exclude: ['src/**/*.svelte.{test,spec}.{js,ts}']
				}
			}
		]
	}
});
