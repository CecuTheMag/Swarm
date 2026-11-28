<script lang="ts">
	import { onMount } from 'svelte';
	import { onNavigate } from '$app/navigation';
	import { account } from '#lib/account.svelte.ts';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import '@fontsource-variable/inter';
	import '../app.css';
	import favicon from '#lib/assets/favicon.svg';
	import Logo from '#lib/ui/Logo.svelte';
	import TabBar from '#lib/ui/TabBar.svelte';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();

	const links = $derived([
		{ href: resolve('/') + '#how', label: 'Как работи', path: null },
		{ href: resolve('/') + '#organisations', label: 'За организации', path: null },
		account.me
			? { href: resolve('/app'), label: 'Конзола', path: '/app' }
			: { href: resolve('/login'), label: 'Вход', path: '/login' }
	]);
	onMount(() => void account.check().catch(() => null));
	const here = (p: string | null) => (p && path.startsWith(p) ? 'page' : undefined);

	const path = $derived(page.url.pathname);
	// The projector page: dark, edge to edge, no footer.
	const stage = $derived(path.startsWith('/render/') || /^\/app\/n\/[^/]+\/live$/.test(path));

	// Page changes cross-fade where the browser supports View Transitions.
	onNavigate((navigation) => {
		if (!document.startViewTransition || matchMedia('(prefers-reduced-motion: reduce)').matches) {
			return;
		}
		return new Promise((done) => {
			document.startViewTransition(async () => {
				done();
				await navigation.complete;
			});
		});
	});
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
	<title>Рояк — суперкомпютър от устройствата около нас</title>
</svelte:head>

<div class="app" class:theme-dark={stage} class:stage>
	<a class="skip" href="#content">Към съдържанието</a>
	<div class="ambient" aria-hidden="true"></div>

	<header class="bar">
		<nav class="glass inner" aria-label="Основна навигация">
			<a class="brand" href={resolve('/')} aria-label="Рояк, начало">
				<Logo size={26} />
				<span>Рояк</span>
			</a>

			<ul class="links">
				{#each links as link (link.href)}
					<li>
						<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- resolve() builds the path; only a #section is added -->
						<a href={link.href} aria-current={here(link.path)}>{link.label}</a>
					</li>
				{/each}
			</ul>

			<a class="button sm cta" href={resolve('/worker')}>Включи се</a>
		</nav>
	</header>

	<div id="content" class="content" tabindex="-1">
		{@render children()}
	</div>

	{#if !stage}
		<TabBar />
		<footer class="foot">
			<div class="foot-inner">
				<div class="foot-brand">
					<a class="brand" href={resolve('/')} aria-label="Рояк, начало">
						<Logo size={22} />
						<span>Рояк</span>
					</a>
					<p class="muted">
						Доброволен суперкомпютър в браузъра. Отворен код, без инсталиране, без лични данни.
					</p>
				</div>
				<ul class="foot-links">
					<li><a href={resolve('/worker')}>Включи се</a></li>
					<li><a href={resolve('/join')}>Включи се с код</a></li>
					<li><a href={resolve('/app')}>Конзола</a></li>
					<li><a href={resolve('/selftest')}>Самопроверка</a></li>
					<li><a href={resolve('/guide')}>Ръководство</a></li>
					<li><a href={resolve('/docs/api')}>Програмен интерфейс</a></li>
					<li>
						<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- resolve() builds the path; only a #section is added -->
						<a href={resolve('/') + '#safety'}>Безопасност</a>
					</li>
				</ul>
				<p class="legal">
					© {new Date().getFullYear()} Рояк · GPL-3.0
				</p>
			</div>
		</footer>
	{/if}
</div>

<style>
	.app {
		min-height: 100dvh;
		display: flex;
		flex-direction: column;
	}
	.content {
		flex: 1;
		display: flex;
		flex-direction: column;
		outline: none;
	}
	.content > :global(main) {
		flex: 1;
		width: 100%;
	}

	/* ---------- Header ---------- */
	.bar {
		position: sticky;
		top: 0;
		z-index: 50;
		padding: 10px var(--gutter) 0;
		padding-top: max(10px, env(safe-area-inset-top));
	}
	.inner {
		max-width: var(--page);
		margin: 0 auto;
		display: flex;
		align-items: center;
		gap: var(--s-4);
		padding: 8px 8px 8px 18px;
		border-radius: var(--pill);
	}
	.brand {
		display: inline-flex;
		align-items: center;
		gap: 10px;
		color: var(--text);
		text-decoration: none;
		font-weight: 750;
		font-size: 1.125rem;
		letter-spacing: -0.02em;
		margin-right: auto;
	}
	.links {
		display: flex;
		gap: 4px;
		list-style: none;
		margin: 0;
		padding: 0;
	}
	.links a {
		display: block;
		padding: 8px 14px;
		border-radius: var(--pill);
		color: var(--muted);
		text-decoration: none;
		font-size: var(--text-sm);
		font-weight: 500;
		transition:
			color var(--fast) var(--ease),
			background-color var(--fast) var(--ease);
	}
	@media (hover: hover) {
		.links a:hover {
			color: var(--text);
			background: rgb(118 118 128 / 0.1);
		}
		.foot-links a:hover {
			color: var(--text);
		}
	}
	.links a[aria-current='page'] {
		color: var(--text);
		background: rgb(118 118 128 / 0.14);
	}
	/* ---------- Phone: the tab bar navigates; the header keeps only the mark ---------- */
	@media (max-width: 760px) {
		.links,
		.cta {
			display: none;
		}
		.bar {
			padding: max(8px, env(safe-area-inset-top)) var(--gutter) 0;
		}
		.inner {
			padding: 6px 14px;
			min-height: 44px;
		}
		.brand {
			font-size: 1.0625rem;
		}
		/* Room for the floating tab bar and the home indicator. */
		.content {
			padding-bottom: calc(84px + env(safe-area-inset-bottom));
		}
		.foot {
			display: none;
		}
	}

	/* ---------- Footer ---------- */
	.foot {
		margin-top: var(--s-8);
		padding: 0 var(--gutter) max(var(--s-6), env(safe-area-inset-bottom));
	}
	.foot-inner {
		max-width: var(--page);
		margin: 0 auto;
		padding-top: var(--s-6);
		border-top: 1px solid var(--border);
		display: grid;
		gap: var(--s-5);
		grid-template-columns: 1.4fr 1fr;
	}
	.foot-brand p {
		margin-top: var(--s-3);
		max-width: 42ch;
		font-size: var(--text-sm);
	}
	.foot-links {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 10px var(--s-5);
		font-size: var(--text-sm);
	}
	.foot-links a {
		color: var(--muted);
		text-decoration: none;
	}
	.legal {
		grid-column: 1 / -1;
		margin: 0;
		color: var(--faint);
		font-size: var(--text-xs);
	}
	@media (max-width: 640px) {
		.foot-inner {
			grid-template-columns: 1fr;
		}
	}

	/* ---------- Projector ---------- */
	.stage .bar {
		padding-top: 8px;
	}
	.stage .inner {
		max-width: min(1800px, 100%);
	}
</style>
