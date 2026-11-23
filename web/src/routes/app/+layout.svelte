<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { onMount } from 'svelte';
	import { account } from '#lib/account.svelte.ts';
	import { networks } from '#lib/console/networks.svelte.ts';
	import Icon from '#lib/ui/Icon.svelte';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();
	let ready = $state(false);

	onMount(async () => {
		const me = await account.refresh().catch(() => null);
		if (!me) {
			// eslint-disable-next-line svelte/no-navigation-without-resolve -- resolve() builds the path; only a query is added
			await goto(`${resolve('/login')}?next=${encodeURIComponent(page.url.pathname)}`, {
				replaceState: true
			});
			return;
		}
		await networks.reload();
		ready = true;
	});

	async function logout() {
		await account.logout();
		await goto(resolve('/login'));
	}

	const path = $derived(page.url.pathname);
	// The live view takes the whole screen.
	const fullscreen = $derived(path.endsWith('/live'));
</script>

{#if ready && account.me}
	{#if fullscreen}
		{@render children()}
	{:else}
		<div class="console">
			<aside class="side glass" class:index={path === resolve('/app')} aria-label="Мрежи">
				<div class="side-head">
					<span class="eyebrow">Моите мрежи</span>
				</div>
				<nav class="nets">
					{#each networks.list as n (n.id)}
						<a
							href={resolve('/app/n/[id]', { id: n.id })}
							aria-current={path.startsWith(resolve('/app/n/[id]', { id: n.id }))
								? 'page'
								: undefined}
							data-testid="nav-network"
						>
							<span class="dot-wrap"><span class="dot" class:on={n.devices_online > 0}></span></span
							>
							<span class="net-name">{n.name}</span>
							<span class="count">{n.devices_online}</span>
						</a>
					{/each}
				</nav>
				<a class="button secondary sm new" href={resolve('/app')}>
					<Icon name="bolt" size={16} /> Нова мрежа
				</a>
				<div class="me">
					{#if account.me.is_admin}
						<a
							href={resolve('/app/admin')}
							aria-current={path === resolve('/app/admin') ? 'page' : undefined}
						>
							<Icon name="shield" size={16} /> Администрация
						</a>
					{/if}
					<span class="muted who">{account.me.name}<br /><small>{account.me.email}</small></span>
					<button class="ghost sm" onclick={logout} data-testid="logout">Изход</button>
				</div>
			</aside>
			<div class="content">
				{@render children()}
			</div>
		</div>
	{/if}
{:else}
	<main class="loading" aria-busy="true"><span class="spinner"></span></main>
{/if}

<style>
	.console {
		max-width: var(--page);
		width: 100%;
		margin: 0 auto;
		padding: var(--s-5) var(--gutter) var(--s-8);
		display: grid;
		grid-template-columns: 250px minmax(0, 1fr);
		gap: var(--s-5);
		align-items: start;
	}
	.side {
		position: sticky;
		top: 88px;
		display: grid;
		gap: var(--s-3);
		padding: var(--s-4);
		border-radius: var(--radius-lg);
	}
	.side-head .eyebrow {
		margin: 0;
	}
	.nets {
		display: grid;
		gap: 2px;
	}
	.nets a,
	.me a {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 9px 10px;
		border-radius: 10px;
		color: var(--text);
		text-decoration: none;
		font-size: var(--text-sm);
		font-weight: 500;
		transition: background-color var(--fast) var(--ease);
	}
	.nets a:hover,
	.me a:hover {
		background: rgb(118 118 128 / 0.1);
	}
	.nets a[aria-current='page'],
	.me a[aria-current='page'] {
		background: rgb(118 118 128 / 0.16);
		font-weight: 650;
	}
	.dot-wrap {
		display: grid;
		place-items: center;
		width: 10px;
	}
	.nets .dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: rgb(118 118 128 / 0.4);
	}
	.nets .dot.on {
		background: var(--ok);
	}
	.net-name {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.count {
		color: var(--muted);
		font-variant-numeric: tabular-nums;
		font-size: var(--text-xs);
	}
	.new {
		justify-self: start;
	}
	.me {
		display: grid;
		gap: var(--s-2);
		border-top: 1px solid var(--border);
		padding-top: var(--s-3);
		margin-top: var(--s-2);
	}
	.who {
		font-size: var(--text-sm);
		overflow-wrap: anywhere;
		padding: 0 10px;
	}
	.me button {
		justify-self: start;
	}
	.content {
		min-width: 0;
	}
	.loading {
		display: grid;
		place-items: center;
		min-height: 50vh;
	}
	.spinner {
		width: 28px;
		height: 28px;
		border-radius: 50%;
		border: 3px solid rgb(118 118 128 / 0.25);
		border-top-color: var(--brand);
		animation: turn 0.9s linear infinite;
	}
	@keyframes turn {
		to {
			transform: rotate(360deg);
		}
	}
	@media (max-width: 900px) {
		.console {
			grid-template-columns: minmax(0, 1fr);
		}
		.side {
			position: static;
		}
		.nets {
			display: flex;
			gap: 6px;
			overflow-x: auto;
			padding-bottom: 4px;
		}
		.nets a {
			flex: none;
			border: 1px solid var(--border);
		}
		.me {
			grid-auto-flow: column;
			justify-content: start;
			align-items: center;
		}
		.who {
			display: none;
		}
	}

	/* Phones: the networks are the console's first screen and each network screen has a
	   back link, so the panel only carries the account, at the end of the first screen. */
	@media (max-width: 760px) {
		.side {
			display: none;
		}
		.side.index {
			display: grid;
			order: 2;
			margin-top: var(--s-6);
		}
		.side.index .side-head,
		.side.index .nets,
		.side.index .new {
			display: none;
		}
		.side.index .me {
			border-top: 0;
			margin: 0;
			padding: 0;
		}
		.side.index .who {
			display: block;
		}
	}
</style>
