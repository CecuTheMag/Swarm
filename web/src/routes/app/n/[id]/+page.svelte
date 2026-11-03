<script lang="ts">
	import { onDestroy } from 'svelte';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { renderSVG } from 'uqr';
	import { LiveDashboard } from '#lib/dashboard/live.svelte.ts';
	import SwarmMap from '#lib/dashboard/SwarmMap.svelte';
	import { caughtDevices, sortedJobs, unitsPerSecond } from '#lib/dashboard/state.ts';
	import { api, failureText } from '#lib/api.ts';
	import { can, useNetwork } from '#lib/console/networks.svelte.ts';
	import CopyField from '#lib/ui/CopyField.svelte';
	import Icon from '#lib/ui/Icon.svelte';
	import type { NetworkView } from '#lib/protocol/NetworkView.ts';

	const network = useNetwork();
	const id = $derived(page.params.id!);

	// This network's live events (members only).
	let live = $state<LiveDashboard | null>(null);
	$effect(() => {
		const l = new LiveDashboard(undefined, `/ws/networks/${id}/live`);
		l.start();
		live = l;
		return () => l.stop();
	});
	onDestroy(() => live?.stop());

	const view = $derived(network.view!);
	const workers = $derived(live ? Object.values(live.state.workers) : []);
	const caught = $derived(live ? caughtDevices(live.state, live.now) : []);
	const rate = $derived(live ? unitsPerSecond(live.state, live.now) : 0);
	const jobs = $derived(live ? sortedJobs(live.state) : []);
	const running = $derived(jobs.find((j) => j.status === 'running' || j.status === 'queued'));

	const joinUrl = $derived(
		new URL(
			view.public ? resolve('/worker') : resolve('/join/[code]', { code: view.join_code }),
			page.url.origin
		).href
	);
	const qr = $derived(renderSVG(joinUrl, { ecc: 'M', border: 2 }));

	let error = $state<string | null>(null);
	async function togglePause() {
		error = null;
		try {
			network.set(
				await api<NetworkView>(`/api/networks/${id}`, {
					method: 'PATCH',
					body: { paused: !view.paused }
				})
			);
		} catch (e) {
			error = failureText(e);
		}
	}
</script>

<svelte:head>
	<title>{view.name} · Рояк</title>
</svelte:head>

<div class="grid-2">
	<section class="card live-card">
		<div class="row">
			<h2>На живо</h2>
			<a class="button secondary sm" href={resolve('/app/n/[id]/live', { id })}>
				<Icon name="image" size={16} /> Цял екран
			</a>
		</div>
		<div class="stats">
			<div class="stat">
				<div class="value" data-testid="overview-online">{workers.length}</div>
				<div class="label">устройства на линия</div>
			</div>
			<div class="stat">
				<div class="value">{rate.toFixed(rate < 10 ? 1 : 0)}</div>
				<div class="label">части в секунда</div>
			</div>
			<div class="stat">
				<div class="value">{view.devices_total}</div>
				<div class="label">устройства общо</div>
			</div>
		</div>
		{#if workers.length > 0 || caught.length > 0}
			<SwarmMap {workers} {caught} />
		{:else}
			<div class="empty">
				<Icon name="phone" size={28} />
				<p>Още няма включени устройства. Отворете връзката за включване на телефон или компютър.</p>
			</div>
		{/if}
		{#if running}
			<a class="job" href={resolve('/app/n/[id]/jobs', { id })}>
				<span
					><Icon name={running.template === 'pt' ? 'image' : 'atom'} size={16} />
					{running.template === 'pt' ? 'Изображение' : 'Числото π'}</span
				>
				<div class="progress">
					<div style:width="{(100 * running.units_done) / running.units_total}%"></div>
				</div>
				<span class="muted">{Math.round((100 * running.units_done) / running.units_total)} %</span>
			</a>
		{/if}
		{#if can(view.role, 'admin')}
			<div class="row actions">
				<button
					class={view.paused ? '' : 'secondary'}
					onclick={togglePause}
					data-testid="pause-network"
				>
					<Icon name={view.paused ? 'power' : 'pause'} size={16} />
					{view.paused ? 'Пусни мрежата' : 'Пауза на мрежата'}
				</button>
				<a class="button secondary" href={resolve('/app/n/[id]/jobs', { id })}>Нова задача</a>
			</div>
		{/if}
		{#if error}<p class="notice bad" role="alert">{error}</p>{/if}
	</section>

	<section class="card add">
		<h2>Добави устройства</h2>
		<p class="muted">
			Сканирайте кода с телефон или отворете връзката на компютър. Устройството се включва в тази
			мрежа{view.join_mode === 'approval' ? ' и чака вашето одобрение' : ''}.
		</p>
		<!-- The SVG is generated locally from this network's own link. -->
		<!-- eslint-disable-next-line svelte/no-at-html-tags -->
		<div class="qr" data-testid="join-qr" data-url={joinUrl}>{@html qr}</div>
		<CopyField value={joinUrl} label="Връзка за включване" testid="join-link" />
		{#if !view.public}
			<p class="muted small">
				Без камера (компютри в кабинет): отворете <code>{page.url.host}/join</code> и въведете кода
				<code data-testid="join-code">{view.join_code}</code>. Можете да смените връзката в
				„Правила“.
			</p>
		{/if}
	</section>
</div>

<style>
	.grid-2 {
		display: grid;
		grid-template-columns: minmax(0, 1.5fr) minmax(0, 1fr);
		gap: var(--s-4);
		align-items: start;
	}
	h2 {
		font-size: var(--text-lg);
		margin: 0;
	}
	.live-card,
	.add {
		display: grid;
		gap: var(--s-4);
	}
	.row {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: var(--s-3);
		flex-wrap: wrap;
	}
	.actions {
		justify-content: flex-start;
	}
	.stats {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: var(--s-3);
	}
	.empty {
		display: grid;
		justify-items: center;
		gap: var(--s-2);
		padding: var(--s-6) var(--s-4);
		text-align: center;
		color: var(--faint);
		border: 1px dashed var(--border-strong);
		border-radius: var(--radius);
	}
	.empty p {
		margin: 0;
		max-width: 36ch;
	}
	.job {
		display: grid;
		grid-template-columns: auto 1fr auto;
		align-items: center;
		gap: var(--s-3);
		color: var(--text);
		text-decoration: none;
		font-size: var(--text-sm);
		font-weight: 600;
	}
	.job span:first-child {
		display: inline-flex;
		gap: 6px;
		align-items: center;
	}
	.qr {
		width: min(220px, 100%);
		justify-self: center;
		background: #fff;
		border-radius: var(--radius);
		line-height: 0;
		padding: 4px;
	}
	.qr :global(svg) {
		width: 100%;
		height: auto;
	}
	.small {
		font-size: var(--text-xs);
		margin: 0;
	}
	@media (max-width: 1000px) {
		.grid-2 {
			grid-template-columns: minmax(0, 1fr);
		}
	}
</style>
