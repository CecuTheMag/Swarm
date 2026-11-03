<script lang="ts">
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { api, failureText } from '#lib/api.ts';
	import { can, useNetwork } from '#lib/console/networks.svelte.ts';
	import SweepCurves from '#lib/console/SweepCurves.svelte';
	import SweepHeatmap from '#lib/console/SweepHeatmap.svelte';
	import ConfirmButton from '#lib/ui/ConfirmButton.svelte';
	import Icon from '#lib/ui/Icon.svelte';
	import type { JobSummary } from '#lib/protocol/JobSummary.ts';
	import type { NnJobParams } from '#lib/protocol/NnJobParams.ts';
	import type { NnResult } from '#lib/protocol/NnResult.ts';
	import type { NnRow } from '#lib/protocol/NnRow.ts';

	// A neural network sweep (spec 0011): which settings trained best, filling in as
	// the network's devices finish their parts.
	const network = useNetwork();
	const id = $derived(page.params.id!);
	const jobId = $derived(page.params.job!);
	const admin = $derived(network.view ? can(network.view.role, 'admin') : false);

	let job = $state<JobSummary | null>(null);
	let board = $state<NnResult | null>(null);
	let error = $state<string | null>(null);
	let missing = $state(false);

	const base = $derived(`/api/networks/${id}/jobs/${jobId}`);
	async function load() {
		try {
			[job, board] = await Promise.all([
				api<JobSummary>(base),
				api<NnResult>(`${base}/leaderboard`)
			]);
			error = null;
		} catch (e) {
			if (failureText(e) === 'not found') missing = true;
			else error = failureText(e);
		}
	}
	// Every two seconds while it runs; once it is finished the board no longer changes.
	$effect(() => {
		void load();
		const timer = setInterval(() => {
			if (!job || job.status === 'queued' || job.status === 'running') void load();
		}, 2000);
		return () => clearInterval(timer);
	});

	const params = $derived(job?.params as NnJobParams | undefined);
	const rows = $derived(board?.rows ?? []);
	const best = $derived(rows[0] as NnRow | undefined);
	const running = $derived(job?.status === 'queued' || job?.status === 'running');
	let all = $state(false);
	const listed = $derived(all ? rows : rows.slice(0, 12));

	const pct = (a: number) => `${(a * 100).toFixed(1)} %`;
	const shape = (r: NnRow) =>
		r.layers === 1
			? `1 скрит слой със ${r.hidden} неврона`
			: `${r.layers} скрити слоя по ${r.hidden} неврона`;
	const statusText: Record<string, string> = {
		queued: 'чака устройства',
		running: 'обучава се',
		done: 'готова',
		failed: 'неуспешна'
	};

	async function stop() {
		try {
			await api(`${base}/cancel`, { body: {} });
			await load();
		} catch (e) {
			error = failureText(e);
		}
	}
</script>

<svelte:head>
	<title>Невронна мрежа · {network.view?.name} · Рояк</title>
</svelte:head>

<a class="back" href={resolve('/app/n/[id]/jobs', { id })}><Icon name="arrow" size={14} /> Задачи</a
>

{#if missing}
	<div class="card">
		<h2>Задачата не е намерена</h2>
		<p class="muted">Може да е изтрита или да е от друга мрежа.</p>
	</div>
{:else if job && params}
	<section class="card head" data-testid="sweep" data-status={job.status}>
		<div class="title">
			<span class="icon"><Icon name="network" size={22} /></span>
			<div>
				<h2>Невронна мрежа · ръкописни цифри</h2>
				<p class="muted">
					{rows.length} от {board ? Math.round(job.units_total / params.seeds) : '…'} настройки ·
					{job.units_done}/{job.units_total} обучения
				</p>
			</div>
			<span
				class="badge"
				class:live={job.status === 'running'}
				class:ok={job.status === 'done'}
				class:bad={job.status === 'failed'}
				>{job.error === 'cancelled' ? 'спряна' : statusText[job.status]}</span
			>
		</div>
		<div class="progress" aria-hidden="true">
			<div style:width="{(100 * job.units_done) / job.units_total}%"></div>
		</div>
		<div class="actions">
			{#if job.status === 'done' && best}
				<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- an API download, not a page -->
				<a class="button" href="{base}/model.json" download data-testid="download-model"
					>Изтегли модела</a
				>
			{/if}
			{#if admin && running}
				<ConfirmButton label="Спри" confirm="Спри търсенето?" onconfirm={stop} />
			{/if}
		</div>
	</section>

	{#if error}<p class="notice bad" role="alert">{error}</p>{/if}

	<section class="card best" aria-live="polite">
		{#if best}
			<span class="eyebrow">{running ? 'Най-добра досега' : 'Най-добра настройка'}</span>
			<div class="hero" data-testid="best-accuracy">{pct(best.accuracy)}</div>
			<p class="what">
				{shape(best)} · скорост на учене {best.learning_rate} · партида
				{best.batch}
			</p>
			<p class="muted small">
				Познава толкова от {board?.validation} цифри, които мрежата не е виждала при обучението
				{best.runs > 1
					? `(средно от ${best.runs} обучения, ${pct(best.min)}–${pct(best.max)})`
					: ''}. Случайно познаване: 10 %.
			</p>
		{:else}
			<span class="eyebrow">Най-добра настройка</span>
			<p class="muted">
				{running
					? 'Устройствата обучават първите настройки. Класацията се появява след няколко секунди.'
					: 'Няма завършени обучения.'}
			</p>
		{/if}
	</section>

	<div class="charts">
		<section class="card">
			<h3>Точност по настройки</h3>
			<p class="muted small">Най-добрата точност за всяка двойка неврони и скорост на учене.</p>
			<SweepHeatmap {rows} learningRates={params.learning_rate} hidden={params.hidden} />
		</section>
		<section class="card">
			<h3>Как се учат най-добрите</h3>
			<SweepCurves {rows} />
		</section>
	</div>

	<section class="card">
		<h3>Класация</h3>
		{#if rows.length === 0}
			<p class="muted">Още няма завършени обучения.</p>
		{:else}
			<div class="table-wrap">
				<table>
					<thead>
						<tr>
							<th>#</th>
							<th title="скрити слоеве × неврони в слой">Мрежа</th>
							<th>Скорост</th>
							<th class="wide">Партида</th>
							<th>Точност</th>
							<th class="wide">Обучения</th>
						</tr>
					</thead>
					<tbody>
						{#each listed as r, i (r.config)}
							<tr data-testid="sweep-row">
								<td class="num">{i + 1}</td>
								<td class="num">{r.layers} × {r.hidden}</td>
								<td class="num">{r.learning_rate}</td>
								<td class="num wide">{r.batch}</td>
								<td>
									<div class="acc">
										<span class="num">{pct(r.accuracy)}</span>
										<span class="meter"><span style:width="{r.accuracy * 100}%"></span></span>
									</div>
								</td>
								<td class="num muted wide">{r.runs}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
			{#if rows.length > 12}
				<button class="ghost sm" onclick={() => (all = !all)}
					>{all ? 'Покажи първите 12' : `Покажи всички ${rows.length}`}</button
				>
			{/if}
		{/if}
		<p class="muted small trust">
			<Icon name="shield" size={14} /> Всяко обучение е повторено на две различни устройства и съвпада
			бит по бит, затова никое устройство не може да подмени резултат.
		</p>
	</section>
{:else}
	<p class="muted">Зареждане…</p>
{/if}

<style>
	section + section,
	.charts,
	.charts + section {
		margin-top: var(--s-4);
	}
	.back {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		margin-bottom: var(--s-3);
		font-size: var(--text-sm);
		text-decoration: none;
	}
	.back :global(svg) {
		transform: rotate(180deg);
	}
	h2,
	h3 {
		margin: 0;
	}
	h2 {
		font-size: var(--text-lg);
	}
	h3 {
		font-size: var(--text-base);
		margin-bottom: var(--s-2);
	}
	.head {
		display: grid;
		gap: var(--s-3);
	}
	.title {
		display: flex;
		align-items: center;
		gap: var(--s-3);
		flex-wrap: wrap;
	}
	.title > div {
		flex: 1;
		min-width: 0;
	}
	.title p {
		margin: 2px 0 0;
		font-size: var(--text-sm);
	}
	.icon {
		display: grid;
		place-items: center;
		width: 44px;
		height: 44px;
		border-radius: 12px;
		background: var(--brand-soft);
		color: var(--brand-ink);
	}
	.actions {
		display: flex;
		gap: var(--s-2);
		flex-wrap: wrap;
	}
	.actions:empty {
		display: none;
	}
	.best {
		text-align: center;
		display: grid;
		justify-items: center;
		gap: var(--s-1);
	}
	.hero {
		font-size: clamp(2.6rem, 8vw, 4rem);
		font-weight: 750;
		letter-spacing: -0.04em;
		line-height: 1.05;
		font-variant-numeric: tabular-nums;
	}
	.what {
		margin: 0;
		font-weight: 600;
	}
	.small {
		font-size: var(--text-xs);
		margin: 0;
	}
	.best .small {
		max-width: 52ch;
	}
	.charts {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 340px), 1fr));
		gap: var(--s-4);
	}
	.charts .card {
		margin: 0;
		min-width: 0;
	}
	.num {
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}
	.acc {
		display: flex;
		align-items: center;
		gap: 8px;
		min-width: 120px;
	}
	.meter {
		flex: 1;
		height: 6px;
		border-radius: 3px;
		background: rgb(118 118 128 / 0.14);
		overflow: hidden;
	}
	.meter span {
		display: block;
		height: 100%;
		border-radius: 3px;
		background: var(--brand);
	}
	/* Phones: the columns that decide the ranking only. */
	@media (max-width: 640px) {
		.wide {
			display: none;
		}
		.acc {
			min-width: 0;
		}
		.meter {
			min-width: 28px;
		}
		th,
		td {
			padding-inline: 6px;
		}
	}
	@media (max-width: 360px) {
		.meter {
			display: none;
		}
	}
	.trust {
		display: flex;
		gap: 6px;
		align-items: flex-start;
		margin-top: var(--s-3);
	}
</style>
