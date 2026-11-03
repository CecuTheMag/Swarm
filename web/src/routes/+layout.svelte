<script lang="ts">
	import { onNavigate } from '$app/navigation';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import '@fontsource-variable/inter';
	import '../app.css';
	import favicon from '#lib/assets/favicon.svg';
	import Logo from '#lib/ui/Logo.svelte';
	import Icon from '#lib/ui/Icon.svelte';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();

	const links = [
		{ href: resolve('/') + '#how', label: 'Как работи', path: null },
		{ href: resolve('/dashboard'), label: 'Табло', path: '/dashboard' },
		{ href: resolve('/selftest'), label: 'Самопроверка', path: '/selftest' }
	];

	let menuOpen = $state(false);
	const path = $derived(page.url.pathname);
	// The projector page: dark, edge to edge, no footer.
	const stage = $derived(path.startsWith('/render/'));

	// Page changes cross-fade where the browser supports View Transitions.
	onNavigate((navigation) => {
		menuOpen = false;
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

	function onKey(event: KeyboardEvent) {
		if (event.key === 'Escape') menuOpen = false;
	}
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
	<title>Рояк — суперкомпютър от устройствата около нас</title>
</svelte:head>

<svelte:window onkeydown={onKey} />

<div class="app" class:theme-dark={stage} class:stage>
	<a class="skip" href="#content">Към съдържанието</a>
	<div class="ambient" aria-hidden="true"></div>

	<header class="bar" class:open={menuOpen}>
		<nav class="glass inner" aria-label="Основна навигация">
			<a class="brand" href={resolve('/')} aria-label="Рояк, начало">
				<Logo size={26} />
				<span>Рояк</span>
			</a>

			<ul class="links">
				{#each links as link (link.href)}
					<li>
						<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- resolve() builds the path; only a #section is added -->
						<a href={link.href} aria-current={link.path && path === link.path ? 'page' : undefined}>
							{link.label}
						</a>
					</li>
				{/each}
			</ul>

			<a class="button sm cta" href={resolve('/worker')}>Включи се</a>

			<button
				class="ghost toggle"
				aria-expanded={menuOpen}
				aria-controls="mobile-menu"
				aria-label={menuOpen ? 'Затвори менюто' : 'Меню'}
				onclick={() => (menuOpen = !menuOpen)}
			>
				<Icon name={menuOpen ? 'close' : 'menu'} size={22} />
			</button>
		</nav>

		<div id="mobile-menu" class="sheet glass" hidden={!menuOpen}>
			<ul>
				{#each links as link, i (link.href)}
					<li style:--i={i}>
						<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- resolve() builds the path; only a #section is added -->
						<a href={link.href} onclick={() => (menuOpen = false)}>{link.label}</a>
					</li>
				{/each}
			</ul>
			<a class="button lg" href={resolve('/worker')} onclick={() => (menuOpen = false)}>
				Включи се <Icon name="arrow" size={18} />
			</a>
		</div>
	</header>

	<div id="content" class="content" tabindex="-1">
		{@render children()}
	</div>

	{#if !stage}
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
					<li><a href={resolve('/dashboard')}>Табло</a></li>
					<li><a href={resolve('/selftest')}>Самопроверка</a></li>
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
	.links a:hover {
		color: var(--text);
		background: rgb(118 118 128 / 0.1);
	}
	.links a[aria-current='page'] {
		color: var(--text);
		background: rgb(118 118 128 / 0.14);
	}
	.toggle {
		display: none;
		width: 44px;
		padding: 0;
		color: var(--text);
	}

	/* ---------- Phone menu ---------- */
	.sheet {
		max-width: var(--page);
		margin: 8px auto 0;
		padding: var(--s-4);
		border-radius: var(--radius-lg);
		display: grid;
		gap: var(--s-4);
		background: var(--glass-strong);
		animation: drop var(--base) var(--ease);
	}
	.sheet[hidden] {
		display: none;
	}
	.sheet ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}
	.sheet li {
		animation: rise var(--slow) var(--ease) both;
		animation-delay: calc(var(--i) * 40ms + 40ms);
	}
	.sheet li a {
		display: block;
		padding: 14px 8px;
		color: var(--text);
		text-decoration: none;
		font-size: 1.375rem;
		font-weight: 650;
		letter-spacing: -0.02em;
		border-bottom: 1px solid var(--border);
	}
	@keyframes drop {
		from {
			opacity: 0;
			transform: translateY(-8px) scale(0.98);
		}
	}
	@keyframes rise {
		from {
			opacity: 0;
			transform: translateY(8px);
		}
	}

	@media (max-width: 760px) {
		.links,
		.cta {
			display: none;
		}
		.toggle {
			display: inline-flex;
		}
	}
	@media (min-width: 761px) {
		.sheet {
			display: none !important;
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
	.foot-links a:hover {
		color: var(--text);
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
	.stage .inner,
	.stage .sheet {
		max-width: min(1800px, 100%);
	}
</style>
