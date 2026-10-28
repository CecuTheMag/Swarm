import { defineConfig } from 'vitest/config';
import { searchForWorkspaceRoot } from 'vite';
import adapter from '@sveltejs/adapter-node';
import { sveltekit } from '@sveltejs/kit/vite';

export default defineConfig({
	plugins: [
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},
			adapter: adapter()
		})
	],
	server: {
		host: true,
		port: 7701,
		strictPort: true,
		// `just share` serves the dev server over HTTPS at this machine's
		// Tailscale name, so lab PCs and phones get a secure context.
		allowedHosts: ['.ts.net'],
		fs: { allow: [searchForWorkspaceRoot(process.cwd()), '../templates'] },
		// Same origin for the page and the coordinator, so WebGPU's secure-context
		// requirement is met by a single TLS endpoint.
		proxy: {
			'/api': 'http://localhost:7700',
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
