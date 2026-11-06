<script lang="ts">
	// The best timetable a job's searches found (spec 0013), per class and per teacher.
	import { tick } from 'svelte';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { api, failureText } from '#lib/api.ts';
	import { useNetwork } from '#lib/console/networks.svelte.ts';
	import PrintSheets from '#lib/console/PrintSheets.svelte';
	import TimetableGrid from '#lib/console/TimetableGrid.svelte';
	import {
		brokenRules,
		csv,
		decodeSlots,
		grid,
		placeAll,
		type View
	} from '#lib/console/timetable.ts';
	import type { JobSummary } from '#lib/protocol/JobSummary.ts';
	import type { TtJobParams } from '#lib/protocol/TtJobParams.ts';
	import type { TtResult } from '#lib/protocol/TtResult.ts';

	const network = useNetwork();
	const id = $derived(page.params.id!);
	const jobId = $derived(page.params.job!);

	let job = $state<JobSummary | null>(null);
	let error = $state<string | null>(null);
	async function load() {
		try {
			job = await api<JobSummary>(`/api/networks/${id}/jobs/${jobId}`);
			error = null;
		} catch (e) {
			error = failureText(e);
		}
	}
	$effect(() => {
		void load();
		const timer = setInterval(() => {
			if (!job || job.status === 'queued' || job.status === 'running') void load();
		}, 2000);
		return () => clearInterval(timer);
	});

	const params = $derived(job?.params as unknown as TtJobParams | undefined);
	const school = $derived(params?.school);
	const result = $derived(
		job?.status === 'done' && job.result ? (job.result as unknown as TtResult) : null
	);
	const placed = $derived(school && result ? placeAll(school, decodeSlots(result.slots)) : []);
	let view = $state<View>({ kind: 'class', index: 0 });
	const cells = $derived(school && result ? grid(school, placed, view) : []);
	const rules = $derived(result ? brokenRules(result.cost.breakdown) : []);
	const sorted = $derived(result ? [...result.costs].sort((a, b) => a - b) : []);
	const worst = $derived(sorted.at(-1) ?? 1);

	function save(name: string, type: string, text: string) {
		const a = document.createElement('a');
		a.href = URL.createObjectURL(new Blob([text], { type }));
		a.download = name;
		a.click();
		URL.revokeObjectURL(a.href);
	}
	/** What the next print puts on paper: one page per class or teacher. */
	let sheets = $state<View[]>([]);
	async function print(which: 'shown' | 'class' | 'teacher') {
		if (!school) return;
		sheets =
			which === 'shown'
				? [view]
				: (which === 'class' ? school.classes : school.teachers).map((_, index) => ({
						kind: which,
						index
					}));
		await tick();
		window.print();
	}

	const fileName = $derived(`razpisanie-${jobId.slice(0, 8)}`);
	function saveCsv() {
		if (school) save(`${fileName}.csv`, 'text/csv', csv(school, placed));
	}
	function saveJson() {
		if (school && result)
			save(
				`${fileName}.json`,
				'application/json',
				JSON.stringify({ school, slots: decodeSlots(result.slots), cost: result.cost }, null, 1)
			);
	}
</script>

<svelte:head>
	<title>Разписание · {network.view?.name} · Рояк</title>
</svelte:head>

<p><a href={resolve('/app/n/[id]/jobs', { id })}>← Задачи</a></p>
{#if error}<p class="notice bad" role="alert">{error}</p>{/if}

{#if job && school}
	<section class="card">
		<h2>{school.name}</h2>
		{#if !result}
			<p class="muted" data-testid="timetable-progress">
				Търсене {job.units_done} от {job.units_total}: устройствата на мрежата търсят разписания
				едновременно; най-доброто се показва, щом всички свършат.
			</p>
			<div class="progress">
				<div style:width="{(100 * job.units_done) / job.units_total}%"></div>
			</div>
		{:else}
			<div class="summary">
				<span
					class="badge"
					class:ok={result.cost.hard === 0}
					class:bad={result.cost.hard > 0}
					data-testid="timetable-valid"
					data-valid={result.cost.hard === 0}
					>{result.cost.hard === 0
						? 'валидно разписание'
						: 'има нарушени задължителни правила'}</span
				>
				<span class="muted"
					>Оценка <strong data-testid="timetable-cost">{result.cost.total}</strong> (по-малко е
					по-добре), най-доброто от {result.costs.length} търсения.</span
				>
			</div>
			{#if rules.length === 0}
				<p class="ok">Нарушени правила: няма.</p>
			{:else}
				<ul class="rules">
					{#each rules as r (r.label)}
						<li class:bad={r.hard}>{r.count} {r.label}</li>
					{/each}
				</ul>
			{/if}
			<div class="actions">
				<button class="secondary sm" onclick={saveCsv} data-testid="save-csv">Изтегли CSV</button>
				<button class="secondary sm" onclick={saveJson}>Изтегли JSON</button>
				<button class="secondary sm" onclick={() => print('class')} data-testid="print-classes"
					>Печат: всички паралелки</button
				>
				<button class="secondary sm" onclick={() => print('teacher')} data-testid="print-teachers"
					>Печат: всички учители</button
				>
			</div>
		{/if}
	</section>

	{#if result}
		<section class="card">
			<div class="pick">
				<div class="segmented" role="group" aria-label="Изглед">
					<button
						aria-pressed={view.kind === 'class'}
						onclick={() => (view = { kind: 'class', index: 0 })}>Паралелки</button
					>
					<button
						aria-pressed={view.kind === 'teacher'}
						onclick={() => (view = { kind: 'teacher', index: 0 })}
						data-testid="view-teachers">Учители</button
					>
				</div>
				<select
					aria-label={view.kind === 'class' ? 'Паралелка' : 'Учител'}
					data-testid="timetable-pick"
					value={view.index}
					onchange={(e) => (view = { ...view, index: Number(e.currentTarget.value) })}
				>
					{#each view.kind === 'class' ? school.classes : school.teachers as entity, i (i)}
						<option value={i}>{entity.name}</option>
					{/each}
				</select>
			</div>
			<TimetableGrid {cells} days={school.days} kind={view.kind} />
			<button
				class="secondary sm print-one"
				onclick={() => print('shown')}
				data-testid="print-shown"
				>Печат на {view.kind === 'class' ? 'паралелката' : 'учителя'}</button
			>
		</section>
		{#if sheets.length > 0}<PrintSheets {school} {placed} {sheets} />{/if}

		<section class="card">
			<h2>Всички търсения</h2>
			<p class="muted">
				Всяко устройство търси от различно начало. Повечето търсения намират по-добро разписание:
				това е ползата от много устройства.
			</p>
			<div class="spread" data-testid="timetable-spread">
				{#each sorted as c, i (i)}
					<div
						class="bar"
						class:best={i === 0}
						style:height="{Math.max(4, (100 * c) / worst)}%"
						title={String(c)}
					></div>
				{/each}
			</div>
		</section>
	{/if}
{/if}

<style>
	section + section {
		margin-top: var(--s-4);
	}
	h2 {
		font-size: var(--text-lg);
		margin: 0 0 var(--s-3);
	}
	.summary {
		display: flex;
		flex-wrap: wrap;
		gap: var(--s-3);
		align-items: center;
	}
	.rules {
		margin: var(--s-3) 0;
		padding-left: var(--s-5);
	}
	.actions,
	.pick {
		display: flex;
		flex-wrap: wrap;
		gap: var(--s-3);
		align-items: center;
	}
	.pick {
		margin-bottom: var(--s-3);
	}
	.print-one {
		margin-top: var(--s-3);
	}
	.spread {
		display: flex;
		align-items: flex-end;
		gap: 2px;
		height: 120px;
	}
	.bar {
		flex: 1;
		min-width: 2px;
		background: var(--brand-soft, var(--border));
		border-radius: 2px 2px 0 0;
	}
	.bar.best {
		background: var(--brand);
	}
</style>
