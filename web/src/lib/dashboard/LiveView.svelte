<script lang="ts">
	import { onDestroy, onMount, untrack } from 'svelte';
	import { page } from '$app/state';
	import { LiveDashboard } from '#lib/dashboard/live.svelte.ts';
	import SwarmMap from '#lib/dashboard/SwarmMap.svelte';
	import ChaosPanel from '#lib/dashboard/ChaosPanel.svelte';
	import {
		caughtDevices,
		deviceLabel,
		speedUp,
		unitsPerSecond,
		type FeedItem
	} from '#lib/dashboard/state.ts';
	import { fly } from 'svelte/transition';
	import type { JobSummary } from '#lib/protocol/JobSummary.ts';
	import type { PtJobParams } from '#lib/protocol/PtJobParams.ts';
	import { renderSVG } from 'uqr';

	// The live view of one path-tracer job, for a screen or projector: the image as it
	// fills in and sharpens, the devices making it happen (joining, leaving, and caught
	// cheating), and what they achieve together. The public network's (`/render/[id]`)
	// and any network's (`/app/n/[id]/live`) differ only in these addresses.

	let {
		jobId,
		jobPath,
		stream,
		join,
		chaos
	}: {
		jobId: string;
		/** The job's API path; its image is at `${jobPath}/image.png`. */
		jobPath: string;
		/** The network's live events. */
		stream: string;
		/** Where devices join: shown as a QR code. */
		join: string;
		/** The resilience tools' API, and whether they need the stored password. */
		chaos: { base: string; password: boolean } | null;
	} = $props();
	// Fixed for the view's life.
	const id = untrack(() => jobId);
	const path = untrack(() => jobPath);
	const IMAGE_REFRESH_MS = 500;
	const ALARM_MS = 6000;

	let fetched = $state<JobSummary | null>(null);
	let missing = $state(false);
	let imageUrl = $state<string | null>(null);

	// At most one image request in flight and one refresh per IMAGE_REFRESH_MS; a
	// change while a request runs triggers one more afterwards.
	let inFlight = false;
	let again = false;
	let lastFetch = 0;
	function refreshImage() {
		if (inFlight) {
			again = true;
			return;
		}
		inFlight = true;
		setTimeout(
			async () => {
				lastFetch = Date.now();
				try {
					const res = await fetch(`${path}/image.png`, { cache: 'no-store' });
					if (res.ok) {
						const previous = imageUrl;
						imageUrl = URL.createObjectURL(await res.blob());
						if (previous) setTimeout(() => URL.revokeObjectURL(previous), 2000);
					}
				} finally {
					inFlight = false;
					if (again) {
						again = false;
						refreshImage();
					}
				}
			},
			Math.max(0, lastFetch + IMAGE_REFRESH_MS - Date.now())
		);
	}

	const live = new LiveDashboard(
		(event) => {
			if (
				(event.type === 'units_completed' && event.jobs.some((j) => j.job_id === id)) ||
				(event.type === 'job_updated' && event.job.id === id)
			) {
				refreshImage();
			}
		},
		untrack(() => stream)
	);

	onMount(async () => {
		live.start();
		const res = await fetch(path);
		if (res.ok) fetched = (await res.json()) as JobSummary;
		else missing = true;
		refreshImage();
	});
	onDestroy(() => {
		live.stop();
		if (imageUrl) URL.revokeObjectURL(imageUrl);
	});

	// Live updates once the job is in the stream; the fetched copy until then.
	const job = $derived(live.state.jobs[id] ?? fetched);
	const params = $derived(job?.template === 'pt' ? (job.params as unknown as PtJobParams) : null);
	const totalSpp = $derived(params ? params.spp * params.passes : 0);
	const sppNow = $derived(job && params ? (job.units_done / job.units_total) * totalSpp : 0);
	const pathsPerUnit = $derived(
		job && params ? (params.width * params.height * totalSpp) / job.units_total : 0
	);
	const pathsPerSecond = $derived(unitsPerSecond(live.state, live.now) * pathsPerUnit);
	const pathsDone = $derived(job ? job.units_done * pathsPerUnit : 0);
	const together = $derived(speedUp(live.state, live.now));
	const workers = $derived(Object.values(live.state.workers));
	const caught = $derived(caughtDevices(live.state, live.now));
	const caughtIds = $derived(new Set(Object.keys(live.state.caught)));
	const totals = $derived(live.state.totals);
	const elapsed = $derived(
		job ? Math.max(0, ((job.finished_at ?? live.now) - job.created_at) / 1000) : 0
	);
	// A cheater just caught: said loudly, over the image, for a few seconds.
	const alarm = $derived(caught.find((c) => live.now - c.at < ALARM_MS) ?? null);

	// Phones in the room scan this to join.
	const joinUrl = $derived(new URL(join, page.url.origin).href);
	const joinQr = $derived(renderSVG(joinUrl, { ecc: 'M', border: 2 }));

	const formatTime = (s: number) =>
		s < 60 ? `${s.toFixed(0)} с` : `${Math.floor(s / 60)} мин ${Math.round(s % 60)} с`;

	/** "1 част", "5 части". */
	const count = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

	/** 3 200 000 000 → "3,2 млрд." */
	function large(n: number): string {
		for (const [size, name] of [
			[1e12, 'трлн.'],
			[1e9, 'млрд.'],
			[1e6, 'млн.'],
			[1e3, 'хил.']
		] as const) {
			if (n >= size)
				return `${(n / size).toFixed(n >= size * 100 ? 0 : 1).replace('.', ',')} ${name}`;
		}
		return n.toFixed(0);
	}

	function describe(item: FeedItem): string {
		switch (item.kind) {
			case 'joined':
				return item.count === 1
					? `Включи се ${deviceLabel(item.device)}`
					: `Включиха се ${item.count} устройства`;
			case 'left': {
				const who =
					item.count === 1
						? `Изключи се ${deviceLabel(item.device)}`
						: `Изключиха се ${item.count} устройства`;
				return item.units_returned > 0
					? `${who}: ${count(item.units_returned, 'част се връща', 'части се връщат')} в опашката и ${item.units_returned === 1 ? 'се изчислява' : 'се изчисляват'} другаде`
					: who;
			}
			case 'banned':
				return `Хванат измамник (${deviceLabel(item.device)}): ${count(item.wrong_results, 'грешен резултат', 'грешни резултата')} отхвърлени${item.units_redone > 0 ? `, ${count(item.units_redone, 'част', 'части')} се изчисляват наново` : ''}. Устройството е блокирано.`;
		}
	}
</script>

<svelte:head>
	<title>Рояк · рендериране</title>
</svelte:head>

<main class="render">
	{#if missing}
		<p class="bad">Няма такава задача.</p>
	{:else if job && !params}
		<p class="bad">Тази задача не е изображение.</p>
	{:else}
		<div class="stage">
			<div class="picture">
				<div
					class="frame"
					style:aspect-ratio={params ? `${params.width} / ${params.height}` : '16 / 9'}
					data-testid="render-frame"
				>
					{#if imageUrl}
						<img
							src={imageUrl}
							alt="Изображението, изчислявано от мрежата"
							data-testid="render-image"
						/>
					{/if}
					{#if alarm}
						<div class="alarm" role="alert" data-testid="cheater-alarm" transition:fly={{ y: -20 }}>
							<strong>Хванат измамник</strong>
							<span>
								{deviceLabel(alarm.worker.device)} върна {count(
									alarm.wrong_results,
									'грешен резултат',
									'грешни резултата'
								)}. Блокиран е, а работата му се проверява наново.
							</span>
						</div>
					{/if}
				</div>
				<div class="progress" aria-label="Напредък">
					<div style:width="{job ? (100 * job.units_done) / job.units_total : 0}%"></div>
				</div>
			</div>

			<aside class="side">
				<div class="join">
					<!-- The SVG is generated locally from the page's own address. -->
					<!-- eslint-disable-next-line svelte/no-at-html-tags -->
					<div class="qr" data-testid="join-qr" data-url={joinUrl}>{@html joinQr}</div>
					<p>
						<strong>Включете устройство</strong><br />
						Сканирайте кода или отворете
						<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- `join` is a resolved path -->
						<a href={join}>{page.url.host}{join}</a>
					</p>
				</div>

				<section class="swarm">
					<h2>
						Рояк от <span data-testid="render-devices">{workers.length}</span>
						{workers.length === 1 ? 'устройство' : 'устройства'}
					</h2>
					<SwarmMap {workers} {caught} />
				</section>

				{#if chaos}
					<ChaosPanel {workers} {caughtIds} {totals} base={chaos.base} password={chaos.password} />
				{/if}

				<ol class="feed" aria-live="polite" data-testid="feed">
					{#each live.state.feed as item (item.key)}
						<li class={item.kind} in:fly={{ x: 24, duration: 300 }}>
							<span class="when">{formatTime(Math.max(0, (live.now - item.at) / 1000))}</span>
							{describe(item)}
						</li>
					{/each}
				</ol>
			</aside>
		</div>

		<div class="stats">
			<div class="stat">
				<div class="value">{pathsPerSecond > 0 ? (pathsPerSecond / 1e6).toFixed(1) : '—'}</div>
				<div class="label">млн. пътя на светлината в секунда</div>
			</div>
			<div class="stat">
				<div class="value" data-testid="speed-up">
					{together ? `×${together.toFixed(together < 10 ? 1 : 0)}` : '—'}
				</div>
				<div class="label">по-бързо от най-бързото устройство само</div>
			</div>
			<div class="stat">
				<div class="value">{large(pathsDone)}</div>
				<div class="label">изчислени пътя на светлината</div>
			</div>
			<div class="stat">
				<div class="value">{sppNow.toFixed(0)}<span class="of">/{totalSpp}</span></div>
				<div class="label">проби на пиксел</div>
			</div>
			{#if totals.banned > 0}
				<div class="stat caught">
					<div class="value" data-testid="caught-count">{totals.banned}</div>
					<div class="label">
						{totals.banned === 1 ? 'хванат измамник' : 'хванати измамници'}: {count(
							totals.wrongResults,
							'грешен резултат отхвърлен',
							'грешни резултата отхвърлени'
						)}, {count(totals.unitsRedone, 'част изчислена', 'части изчислени')} наново
					</div>
				</div>
			{/if}
			<div class="stat">
				<div class="value" data-testid="render-status" data-status={job?.status}>
					{job?.status === 'done' ? 'готово' : formatTime(elapsed)}
				</div>
				<div class="label">
					{job?.status === 'done' ? `за ${formatTime(elapsed)}` : 'изминало време'}
				</div>
			</div>
		</div>
	{/if}
</main>

<style>
	.render {
		max-width: min(1800px, 100%);
		padding-top: var(--s-4);
		padding-bottom: var(--s-5);
	}
	.stage {
		display: grid;
		gap: 20px;
	}
	/* A projector: the picture on the left, the swarm beside it. */
	@media (min-width: 1000px) {
		.stage {
			grid-template-columns: minmax(0, 1fr) clamp(300px, 27vw, 440px);
		}
		/* As tall as the picture and no taller, so the numbers below stay on screen;
		   the oldest events slip out of view first, and a picture too short for the
		   rest scrolls the column instead of covering the numbers. */
		.side {
			height: 0;
			min-height: 100%;
			overflow-y: auto;
		}
		.feed {
			flex: 1;
			min-height: 0;
			overflow: hidden;
			mask-image: linear-gradient(to bottom, #000 75%, transparent);
		}
	}
	.frame {
		position: relative;
		width: 100%;
		max-height: calc(100vh - 260px);
		margin: 0 auto;
		border-radius: var(--radius);
		overflow: hidden;
		/* Pixels nobody has rendered yet are transparent. */
		background: repeating-conic-gradient(#1c1f26 0% 25%, #252932 0% 50%) 0 0 / 24px 24px;
	}
	.frame img {
		display: block;
		width: 100%;
		height: 100%;
		object-fit: contain;
	}
	.progress {
		margin-top: 10px;
	}
	.alarm {
		position: absolute;
		top: 16px;
		left: 50%;
		translate: -50% 0;
		width: min(92%, 640px);
		padding: 14px 20px;
		border-radius: var(--radius);
		/* A deeper red than the theme's, so the white text stays readable. */
		background: #c4161c;
		color: #fff;
		box-shadow: 0 8px 30px rgb(0 0 0 / 0.45);
		font-size: clamp(0.95rem, 1.5vw, 1.25rem);
	}
	.alarm strong {
		display: block;
		font-size: 1.35em;
	}
	.side {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}
	.join {
		display: flex;
		align-items: center;
		gap: 16px;
		font-size: clamp(1rem, 1.3vw, 1.25rem);
	}
	.join p {
		margin: 0;
		min-width: 0;
		overflow-wrap: anywhere;
	}
	.qr {
		flex: none;
		width: clamp(110px, 10vw, 170px);
		/* Dark modules on white: scanners need the contrast, whatever the theme. */
		background: #fff;
		border-radius: 8px;
		line-height: 0;
	}
	.qr :global(svg) {
		width: 100%;
		height: auto;
	}
	.swarm h2 {
		margin-bottom: 8px;
	}
	.feed {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 6px;
		font-size: 0.9375rem;
	}
	.feed li {
		padding: 8px 10px;
		border-radius: 10px;
		background: var(--glass);
		border: 1px solid var(--glass-border);
		border-left: 4px solid var(--c, var(--border));
	}
	.feed .joined {
		--c: var(--ok);
	}
	.feed .left {
		--c: var(--muted);
	}
	.feed .banned {
		--c: var(--bad);
		font-weight: 600;
	}
	.when {
		float: right;
		margin-left: 8px;
		color: var(--muted);
		font-size: 0.8125rem;
		font-variant-numeric: tabular-nums;
	}
	.stats {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(170px, 1fr));
		gap: 12px;
		margin-top: 18px;
	}
	.stats .stat {
		padding: 12px 18px;
		border-radius: var(--radius);
		background: var(--glass);
		border: 1px solid var(--glass-border);
		box-shadow: inset 0 1px 0 var(--glass-highlight);
		backdrop-filter: blur(var(--glass-blur));
		-webkit-backdrop-filter: blur(var(--glass-blur));
	}
	.stat .value {
		font-size: clamp(1.6rem, 2.8vw, 2.6rem);
		font-weight: 800;
		font-variant-numeric: tabular-nums;
	}
	.stat .of {
		font-size: 0.5em;
		color: var(--muted);
		font-weight: 600;
	}
	.stat .label {
		color: var(--muted);
	}
	.stat.caught .value {
		color: var(--bad);
	}
</style>
