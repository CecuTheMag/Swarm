<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { api, failureText } from '#lib/api.ts';
	import { account } from '#lib/account.svelte.ts';
	import { networks } from '#lib/console/networks.svelte.ts';
	import Icon from '#lib/ui/Icon.svelte';
	import type { NetworkView } from '#lib/protocol/NetworkView.ts';

	let name = $state('');
	let error = $state<string | null>(null);
	let busy = $state(false);

	async function create(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		error = null;
		try {
			const n = await api<NetworkView>('/api/networks', { body: { name } });
			await networks.reload();
			await goto(resolve('/app/n/[id]', { id: n.id }));
		} catch (e) {
			error = failureText(e);
		} finally {
			busy = false;
		}
	}

	const roleText = { owner: 'собственик', admin: 'администратор', viewer: 'наблюдател' };
</script>

<svelte:head>
	<title>Моите мрежи · Рояк</title>
</svelte:head>

<header class="page-head">
	<span class="eyebrow">Конзола</span>
	<h1>Здравейте, {account.me?.name}</h1>
	<p>
		Всяка мрежа е ваша група от устройства: кабинет, лаборатория, екип. Устройствата в нея
		изчисляват само вашите задачи.
	</p>
</header>

{#if networks.list.length > 0}
	<section class="nets">
		{#each networks.list as n (n.id)}
			<a class="card net" href={resolve('/app/n/[id]', { id: n.id })} data-testid="network-tile">
				<div class="net-top">
					<span class="icon-tile"><Icon name={n.public ? 'globe' : 'chip'} size={20} /></span>
					<span class="badge">{roleText[n.role]}</span>
				</div>
				<h2>{n.name}</h2>
				<p class="muted">
					<span class="online" class:on={n.devices_online > 0}></span>
					{n.devices_online} на линия · {n.devices_total} общо
					{#if n.paused}· <strong>на пауза</strong>{/if}
				</p>
			</a>
		{/each}
	</section>
{/if}

<section class="card create">
	<h2>{networks.list.length ? 'Нова мрежа' : 'Създайте първата си мрежа'}</h2>
	<p class="muted">
		Дайте ѝ име, което хората ще разпознаят, например „Кабинет 12“ или „Лаборатория по физика“.
	</p>
	<form onsubmit={create}>
		<input
			placeholder="Име на мрежата"
			required
			maxlength="80"
			bind:value={name}
			aria-label="Име на мрежата"
			data-testid="new-network-name"
		/>
		<button type="submit" disabled={busy || !name.trim()}>Създай</button>
	</form>
	{#if error}<p class="notice bad" role="alert">{error}</p>{/if}
</section>

<style>
	.nets {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(min(100%, 240px), 1fr));
		gap: var(--s-4);
		margin-bottom: var(--s-5);
	}
	.net {
		display: grid;
		gap: var(--s-2);
		color: var(--text);
		text-decoration: none;
		transition:
			transform var(--base) var(--ease),
			box-shadow var(--base) var(--ease);
	}
	.net:hover {
		transform: translateY(-3px);
		box-shadow:
			inset 0 1px 0 var(--glass-highlight),
			var(--shadow-lg);
	}
	.net-top {
		display: flex;
		justify-content: space-between;
		align-items: flex-start;
	}
	.net h2 {
		font-size: var(--text-lg);
		margin: var(--s-2) 0 0;
		overflow-wrap: anywhere;
	}
	.net p {
		margin: 0;
		font-size: var(--text-sm);
		display: flex;
		align-items: center;
		gap: 6px;
		flex-wrap: wrap;
	}
	.icon-tile {
		display: inline-grid;
		place-items: center;
		width: 40px;
		height: 40px;
		border-radius: 12px;
		background: var(--brand-soft);
		color: var(--brand-ink);
	}
	.online {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: rgb(118 118 128 / 0.4);
	}
	.online.on {
		background: var(--ok);
	}
	.create h2 {
		font-size: var(--text-lg);
	}
	.create form {
		display: flex;
		gap: var(--s-3);
		margin-top: var(--s-3);
	}
	.create .notice {
		margin: var(--s-3) 0 0;
	}
	@media (max-width: 520px) {
		.create form {
			flex-direction: column;
		}
	}
	/* Phones: the large title carries the screen; no label above it. */
	@media (max-width: 760px) {
		.eyebrow {
			display: none;
		}
	}
</style>
