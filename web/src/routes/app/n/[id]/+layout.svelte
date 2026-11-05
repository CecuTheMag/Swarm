<script lang="ts">
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { CurrentNetwork, can, provideNetwork } from '#lib/console/networks.svelte.ts';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();

	// A new network id (switching in the sidebar) loads that network.
	const current = $derived(new CurrentNetwork(page.params.id!));
	$effect(() => {
		void current.reload();
	});
	// Tabs read it through the context; the getter follows `current`.
	provideNetwork({
		get view() {
			return current.view;
		},
		get missing() {
			return current.missing;
		},
		get id() {
			return current.id;
		},
		reload: () => current.reload(),
		set: (v) => current.set(v)
	} as CurrentNetwork);

	const tabs = $derived.by(() => {
		const id = page.params.id!;
		const role = current.view?.role ?? 'viewer';
		return [
			{ href: resolve('/app/n/[id]', { id }), label: 'Обзор', show: true },
			{ href: resolve('/app/n/[id]/devices', { id }), label: 'Устройства', show: true },
			{ href: resolve('/app/n/[id]/jobs', { id }), label: 'Задачи', show: true },
			{ href: resolve('/app/n/[id]/programs', { id }), label: 'Програми', show: true },
			{ href: resolve('/app/n/[id]/rules', { id }), label: 'Правила', show: true },
			{ href: resolve('/app/n/[id]/members', { id }), label: 'Членове', show: true },
			{ href: resolve('/app/n/[id]/api', { id }), label: 'API', show: can(role, 'admin') },
			{
				href: resolve('/app/n/[id]/resilience', { id }),
				label: 'Устойчивост',
				show: can(role, 'admin')
			}
		].filter((t) => t.show);
	});
	/** A tab stays current on its own sub-pages (a job's page is under „Задачи“). */
	const current_ = (href: string) =>
		page.url.pathname === href ||
		(href !== resolve('/app/n/[id]', { id: page.params.id! }) &&
			page.url.pathname.startsWith(href + '/'));
</script>

{#if page.url.pathname.endsWith('/live')}
	{#if current.view}{@render children()}{/if}
{:else if current.missing}
	<div class="card">
		<h1>Мрежата не е намерена</h1>
		<p class="muted">Може да е изтрита или да нямате достъп до нея.</p>
		<a class="button secondary" href={resolve('/app')}>Към мрежите</a>
	</div>
{:else if current.view}
	<header class="net-head">
		<div>
			<span class="eyebrow">{current.view.public ? 'Общата мрежа' : 'Мрежа'}</span>
			<h1 data-testid="network-title">{current.view.name}</h1>
		</div>
		<div class="head-badges">
			{#if current.view.paused}<span class="badge bad">на пауза</span>{/if}
			<span class="badge" class:live={current.view.devices_online > 0}>
				<span class="dot" class:pulse={current.view.devices_online > 0}></span>
				{current.view.devices_online} на линия
			</span>
		</div>
	</header>
	<nav class="tabs" aria-label="Раздели на мрежата">
		{#each tabs as t (t.href)}
			<a href={t.href} aria-current={current_(t.href) ? 'page' : undefined}>{t.label}</a>
		{/each}
	</nav>
	{@render children()}
{/if}

<style>
	.net-head {
		display: flex;
		justify-content: space-between;
		align-items: flex-end;
		gap: var(--s-3);
		flex-wrap: wrap;
		margin-bottom: var(--s-4);
	}
	.net-head h1 {
		margin: 0;
		overflow-wrap: anywhere;
	}
	.head-badges {
		display: flex;
		gap: var(--s-2);
		flex-wrap: wrap;
	}
	.tabs {
		display: flex;
		gap: 4px;
		overflow-x: auto;
		margin-bottom: var(--s-5);
		padding: 4px;
		border-radius: var(--pill);
		background: rgb(118 118 128 / 0.12);
		width: fit-content;
		max-width: 100%;
		scrollbar-width: none;
	}
	.tabs a {
		flex: none;
		padding: 8px 16px;
		border-radius: var(--pill);
		color: var(--text);
		text-decoration: none;
		font-size: var(--text-sm);
		font-weight: 500;
		transition: background-color var(--base) var(--ease);
	}
	.tabs a:hover {
		background: rgb(118 118 128 / 0.12);
	}
	.tabs a[aria-current='page'] {
		background: var(--text);
		color: var(--bg);
		font-weight: 650;
	}
</style>
