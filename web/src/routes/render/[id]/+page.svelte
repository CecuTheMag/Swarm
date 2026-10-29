<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { LiveDashboard } from '#lib/dashboard/live.svelte.ts';
	import { unitsPerSecond } from '#lib/dashboard/state.ts';
	import type { JobSummary } from '#lib/protocol/JobSummary.ts';
	import type { PtJobParams } from '#lib/protocol/PtJobParams.ts';

	// The projector view of one path-tracer job: the image as it fills in and sharpens,
	// and how many devices are making it happen.

	const id = page.params.id!;
	const IMAGE_REFRESH_MS = 500;

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
					const res = await fetch(`/api/jobs/${id}/image.png`, { cache: 'no-store' });
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

	const live = new LiveDashboard((event) => {
		if (
			(event.type === 'unit_completed' && event.job_id === id) ||
			(event.type === 'job_updated' && event.job.id === id)
		) {
			refreshImage();
		}
	});

	onMount(async () => {
		live.start();
		const res = await fetch(`/api/jobs/${id}`);
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
	const workers = $derived(Object.values(live.state.workers));
	const gpus = $derived(workers.filter((w) => w.device.backend === 'webgpu').length);
	const elapsed = $derived(
		job ? Math.max(0, ((job.finished_at ?? live.now) - job.created_at) / 1000) : 0
	);

	const formatTime = (s: number) =>
		s < 60 ? `${s.toFixed(0)} с` : `${Math.floor(s / 60)} мин ${Math.round(s % 60)} с`;
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
		</div>

		<div class="stats">
			<div class="stat">
				<div class="value" data-testid="render-devices">{workers.length}</div>
				<div class="label">
					устройства{workers.length > 0
						? ` (${gpus} с видеокарта, ${workers.length - gpus} с процесор)`
						: ''}
				</div>
			</div>
			<div class="stat">
				<div class="value">{pathsPerSecond > 0 ? (pathsPerSecond / 1e6).toFixed(1) : '—'}</div>
				<div class="label">млн. пътя на светлината в секунда</div>
			</div>
			<div class="stat">
				<div class="value">{sppNow.toFixed(0)}<span class="of">/{totalSpp}</span></div>
				<div class="label">проби на пиксел</div>
			</div>
			<div class="stat">
				<div class="value" data-testid="render-status" data-status={job?.status}>
					{job?.status === 'done' ? 'готово' : formatTime(elapsed)}
				</div>
				<div class="label">
					{job?.status === 'done' ? `за ${formatTime(elapsed)}` : 'изминало време'}
				</div>
			</div>
		</div>

		<div class="progress" aria-label="Напредък">
			<div style:width="{job ? (100 * job.units_done) / job.units_total : 0}%"></div>
		</div>

		<p class="join">
			Включете устройството си: <a href={resolve('/worker')}>{page.url.host}/worker</a>
		</p>
	{/if}
</main>

<style>
	.render {
		max-width: min(1600px, 100%);
	}
	.frame {
		width: 100%;
		max-height: 72vh;
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
	.stats {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
		gap: 12px;
		margin: 16px 0 10px;
	}
	.stat .value {
		font-size: clamp(1.6rem, 3.2vw, 2.6rem);
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
	.join {
		margin-top: 14px;
		font-size: clamp(1rem, 2vw, 1.4rem);
	}
</style>
