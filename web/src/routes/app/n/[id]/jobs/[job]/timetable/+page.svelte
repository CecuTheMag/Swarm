<script lang="ts">
	// The best timetable a job's searches found (spec 0013), per class and per teacher;
	// edited by hand, with what each move costs said before it is made, kept, printed,
	// and searched again around the hours pinned (spec 0015).
	import { tick } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import { beforeNavigate, goto } from '$app/navigation';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { api, ApiFailure, failureText } from '#lib/api.ts';
	import { can, useNetwork } from '#lib/console/networks.svelte.ts';
	import PrintSheets from '#lib/console/PrintSheets.svelte';
	import { scoreKernels } from '#lib/console/score.ts';
	import TimetableGrid, { type Preview } from '#lib/console/TimetableGrid.svelte';
	import {
		atSlot,
		brokenRules,
		conflicts,
		csv,
		decodeSlots,
		drop,
		encodeSlots,
		grid,
		placeAll,
		withPins,
		type View
	} from '#lib/console/timetable.ts';
	import { check } from '#lib/school/checks.ts';
	import { timetableScorer } from '#lib/tt.ts';
	import ConfirmButton from '#lib/ui/ConfirmButton.svelte';
	import type { JobSummary } from '#lib/protocol/JobSummary.ts';
	import type { TimetableEdit } from '#lib/protocol/TimetableEdit.ts';
	import type { TtCost } from '#lib/protocol/TtCost.ts';
	import type { TtJobParams } from '#lib/protocol/TtJobParams.ts';
	import type { TtResult } from '#lib/protocol/TtResult.ts';

	const network = useNetwork();
	const id = $derived(page.params.id!);
	const jobId = $derived(page.params.job!);
	const admin = $derived(network.view ? can(network.view.role, 'admin') : false);

	let job = $state<JobSummary | null>(null);
	let error = $state<string | null>(null);
	let notice = $state<string | null>(null);
	let busy = $state(false);

	/** The saved edit, once read: null for none. */
	let edit = $state<TimetableEdit | null>(null);
	let editRead = $state(false);
	/** The timetable shown, and the hours pinned by hand: the saved ones until a change. */
	let slots = $state<number[] | null>(null);
	const pins = new SvelteSet<number>();
	let history = $state<{ slots: number[]; pins: number[] }[]>([]);
	let editing = $state(false);
	let selected = $state<number | null>(null);
	let score = $state<((slots: number[]) => TtCost) | null>(null);

	// A new job (after „Търси отново“) starts from nothing on the same page, before it is
	// drawn: the last job's timetable is never shown under the new job's address.
	let shownFor = '';
	$effect.pre(() => {
		if (shownFor === jobId) return;
		shownFor = jobId;
		job = null;
		edit = null;
		editRead = false;
		slots = null;
		pins.clear();
		history = [];
		editing = false;
		selected = null;
		score = null;
		error = null;
		notice = null;
	});

	async function load() {
		try {
			const read = await api<JobSummary>(`/api/networks/${id}/jobs/${jobId}`);
			if (read.id !== jobId) return;
			job = read;
			error = null;
			if (read.status === 'done' && read.result && !editRead) {
				// Read once: a poll still in flight must not undo moves made since.
				editRead = true;
				let saved;
				try {
					saved = await api<TimetableEdit | null>(`/api/networks/${id}/jobs/${jobId}/timetable`);
				} catch (e) {
					editRead = false;
					throw e;
				}
				if (read.id !== jobId) return;
				edit = saved;
				slots = saved
					? decodeSlots(saved.slots)
					: decodeSlots((read.result as unknown as TtResult).slots);
				pins.clear();
				for (const h of saved?.pinned ?? []) pins.add(h);
			}
		} catch (e) {
			error = failureText(e);
		}
	}
	$effect(() => {
		void jobId;
		void load();
		const timer = setInterval(() => {
			const waiting = !job || job.status === 'queued' || job.status === 'running';
			if (waiting || (job?.status === 'done' && !editRead)) void load();
		}, 2000);
		return () => clearInterval(timer);
	});

	const params = $derived(job?.params as unknown as TtJobParams | undefined);
	const school = $derived(params?.school);
	const result = $derived(
		job?.status === 'done' && job.result ? (job.result as unknown as TtResult) : null
	);
	const found = $derived(result ? decodeSlots(result.slots) : null);
	const placed = $derived(school && slots ? placeAll(school, slots) : []);
	let view = $state<View>({ kind: 'class', index: 0 });
	const cells = $derived(school && slots ? grid(school, placed, view) : []);

	const same = (a: readonly number[], b: readonly number[]) =>
		a.length === b.length && a.every((x, i) => x === b[i]);
	const sortedPins = $derived([...pins].sort((a, b) => a - b));
	/** What is saved: the edit, or the timetable found. */
	const savedSlots = $derived(edit ? decodeSlots(edit.slots) : found);
	const dirty = $derived(
		slots !== null &&
			savedSlots !== null &&
			(!same(slots, savedSlots) || !same(sortedPins, edit?.pinned ?? []))
	);
	const moved = $derived(
		new Set(slots && found ? slots.flatMap((s, h) => (s === found[h] ? [] : [h])) : [])
	);
	/** The cost of the timetable shown: the coordinator's while it is as saved. */
	const cost = $derived.by((): TtCost | null => {
		if (!result || !slots) return null;
		if (!dirty) return edit?.cost ?? result.cost;
		return score ? score(slots) : null;
	});
	const rules = $derived(cost ? brokenRules(cost.breakdown) : []);
	const named = $derived(school && cost && cost.hard > 0 ? conflicts(school, placed) : []);
	const sorted = $derived(result ? [...result.costs].sort((a, b) => a - b) : []);
	const worst = $derived(sorted.at(-1) ?? 1);

	/** What putting the picked hour in each cell of the view would change. */
	const preview = $derived.by((): (Preview | null)[][] | null => {
		if (selected === null || !school || !slots || !score || !cost) return null;
		const picked = selected;
		const now = cost;
		return Array.from({ length: school.periods }, (_, period) =>
			Array.from({ length: school.days }, (_, day) => {
				const after = drop(school, placed, slots!, picked, view, day, period);
				if (!after) return null;
				const c = score!(after.slots);
				return { hard: c.hard - now.hard, soft: c.soft - now.soft };
			})
		);
	});
	const pickedHour = $derived(selected === null ? null : (placed[selected] ?? null));

	async function startEditing() {
		error = null;
		if (!score && school) {
			try {
				const k = await scoreKernels();
				score = timetableScorer(k, new TextEncoder().encode(JSON.stringify(school)));
			} catch (e) {
				error = `Оценката на живо не се зареди: ${failureText(e)}`;
				return;
			}
		}
		editing = true;
	}

	function remember() {
		history.push({ slots: slots!.slice(), pins: [...pins] });
	}

	function pick(hour: number) {
		notice = null;
		selected = selected === hour ? null : hour;
	}

	function put(day: number, period: number) {
		if (selected === null || !school || !slots) return;
		const after = drop(school, placed, slots, selected, view, day, period);
		if (after) {
			remember();
			slots = after.slots;
			// The hour put by hand stays there in the next search, unless unpinned.
			pins.add(after.moved[0]);
		}
		selected = null;
	}

	function togglePin(hour: number) {
		remember();
		if (pins.has(hour)) pins.delete(hour);
		else pins.add(hour);
	}

	function undo() {
		const last = history.pop();
		if (!last) return;
		slots = last.slots;
		pins.clear();
		for (const h of last.pins) pins.add(h);
		selected = null;
	}

	function changeView(next: View) {
		selected = null;
		view = next;
	}

	async function save() {
		if (!slots) return;
		busy = true;
		error = null;
		notice = null;
		try {
			edit = await api<TimetableEdit>(`/api/networks/${id}/jobs/${jobId}/timetable`, {
				method: 'PUT',
				body: {
					slots: encodeSlots(slots),
					pinned: sortedPins,
					...(edit ? { version: edit.version } : {})
				}
			});
			notice = 'Запазено.';
		} catch (e) {
			error =
				e instanceof ApiFailure && e.status === 409
					? `${e.message}. Презаредете страницата, за да видите промените; вашите не са запазени.`
					: failureText(e);
		} finally {
			busy = false;
		}
	}

	async function revert() {
		if (!found) return;
		busy = true;
		error = null;
		notice = null;
		try {
			if (edit) {
				await api(`/api/networks/${id}/jobs/${jobId}/timetable`, { method: 'DELETE' });
				edit = null;
			}
			remember();
			slots = found.slice();
			pins.clear();
			selected = null;
			notice = 'Показва се намереното разписание.';
		} catch (e) {
			error = failureText(e);
		} finally {
			busy = false;
		}
	}

	/** Problems the next search would have with the hours pinned here. */
	let againProblems = $state<string[]>([]);
	async function searchAgain() {
		if (!school || !slots || !params || !job) return;
		error = null;
		againProblems = [];
		let next;
		try {
			next = withPins(school, slots, pins);
		} catch (e) {
			error = failureText(e);
			return;
		}
		againProblems = check(next)
			.filter((f) => f.level === 'problem')
			.map((f) => f.text);
		if (againProblems.length > 0) return;
		busy = true;
		try {
			const seed = crypto.getRandomValues(new Uint32Array(1))[0];
			const started = await api<JobSummary>(`/api/networks/${id}/jobs`, {
				body: {
					template: 'tt',
					params: {
						school: next,
						searches: params.searches,
						iterations: params.iterations,
						seed
					},
					...(network.view?.public ? {} : { quorum: job.quorum })
				}
			});
			await goto(resolve('/app/n/[id]/jobs/[job]/timetable', { id, job: started.id }));
		} catch (e) {
			error = failureText(e);
		} finally {
			busy = false;
		}
	}

	beforeNavigate((nav) => {
		if (
			dirty &&
			!busy &&
			nav.type !== 'leave' &&
			!confirm('Има незапазени промени в разписанието. Да се изоставят ли?')
		) {
			nav.cancel();
		}
	});
	function beforeUnload(event: BeforeUnloadEvent) {
		if (dirty) event.preventDefault();
	}

	function download(name: string, type: string, text: string) {
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
		if (school) download(`${fileName}.csv`, 'text/csv', csv(school, placed));
	}
	function saveJson() {
		if (school && slots && cost)
			download(
				`${fileName}.json`,
				'application/json',
				JSON.stringify({ school, slots, cost }, null, 1)
			);
	}
</script>

<svelte:window onbeforeunload={beforeUnload} />

<svelte:head>
	<title>Разписание · {network.view?.name} · Рояк</title>
</svelte:head>

<p><a href={resolve('/app/n/[id]/jobs', { id })}>← Задачи</a></p>
{#if error}<p class="notice bad" role="alert" data-testid="timetable-error">{error}</p>{/if}

{#if job && school}
	<section class="card" data-testid="timetable-job" data-job={job.id}>
		<h2>{school.name}</h2>
		{#if !result || !slots}
			<p class="muted" data-testid="timetable-progress">
				Търсене {job.units_done} от {job.units_total}: устройствата на мрежата търсят разписания
				едновременно; най-доброто се показва, щом всички свършат.
			</p>
			<div class="progress">
				<div style:width="{(100 * job.units_done) / job.units_total}%"></div>
			</div>
		{:else}
			<div class="summary">
				{#if cost}
					<span
						class="badge"
						class:ok={cost.hard === 0}
						class:bad={cost.hard > 0}
						data-testid="timetable-valid"
						data-valid={cost.hard === 0}
						>{cost.hard === 0 ? 'валидно разписание' : 'има нарушени задължителни правила'}</span
					>
					<span class="muted"
						>Оценка <strong data-testid="timetable-cost">{cost.total}</strong> (по-малко е по-добре){#if edit || dirty},
							след ръчни промени; намереното има {result.cost.total}{:else}, най-доброто от {result
								.costs.length} търсения{/if}.</span
					>
				{/if}
				{#if edit || dirty}
					<span class="badge" class:ok={!dirty} data-testid="edit-state" data-dirty={dirty}
						>{dirty ? 'незапазени промени' : 'редактирано'}</span
					>
				{/if}
			</div>
			{#if rules.length === 0}
				<p class="ok">Нарушени правила: няма.</p>
			{:else}
				<ul class="rules" data-testid="timetable-rules">
					{#each rules as r (r.label)}
						<li class:bad={r.hard}>{r.count} {r.label}</li>
					{/each}
				</ul>
			{/if}
			{#if named.length > 0}
				<ul class="conflicts" data-testid="conflicts">
					{#each named as c, i (i)}
						<li class="bad" data-testid="conflict">{c}</li>
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
				{#if admin && !editing}
					<button class="sm" onclick={startEditing} data-testid="edit-timetable">Редактирай</button>
				{/if}
			</div>
		{/if}
	</section>

	{#if result && slots}
		<section class="card">
			{#if editing}
				<div class="toolbar" data-testid="edit-toolbar">
					<button
						type="button"
						class="sm"
						disabled={!dirty || busy}
						onclick={save}
						data-testid="save-edit">Запази</button
					>
					<button
						type="button"
						class="secondary sm"
						disabled={history.length === 0}
						onclick={undo}
						data-testid="undo">Назад</button
					>
					{#if edit || moved.size > 0 || pins.size > 0}
						<ConfirmButton
							label="Върни намереното"
							confirm="Да, върни го"
							testid="revert"
							onconfirm={revert}
						/>
					{/if}
					<button
						type="button"
						class="secondary sm"
						disabled={busy || (pins.size === 0 && !school.lessons.some((l) => l.pinned?.length))}
						onclick={searchAgain}
						data-testid="search-again">Търси отново около закрепените</button
					>
					<button
						type="button"
						class="ghost sm"
						onclick={() => {
							editing = false;
							selected = null;
						}}
						data-testid="edit-done">Готово</button
					>
				</div>
				<p class="muted small" data-testid="edit-help">
					{#if pickedHour}
						Изберете къде да отиде <strong>{pickedHour.class} {pickedHour.subject}</strong>
						({atSlot(pickedHour.day, pickedHour.period)}): всяка клетка показва промяната в
						оценката, а „конфл.“ — нарушени задължителни правила. Час на заета клетка си разменя
						мястото с нея.
						<button
							type="button"
							class="ghost sm"
							onclick={() => togglePin(pickedHour!.hour)}
							data-testid="toggle-pin"
							>{pins.has(pickedHour.hour)
								? 'Откачи (търсенето може да го мести)'
								: '📌 Закрепи'}</button
						>
					{:else}
						Изберете час, за да го преместите. 📌 — закрепен на ръка: следващото търсене го оставя
						там. 🔒 — закрепен в данните на училището. Променени: {moved.size}.
					{/if}
				</p>
				{#if againProblems.length > 0}
					{#each againProblems as p, i (i)}
						<p class="notice bad" data-testid="again-problem">{p}</p>
					{/each}
				{/if}
			{/if}
			{#if notice}<p class="notice ok" role="status" data-testid="timetable-notice">
					{notice}
				</p>{/if}
			<div class="pick">
				<div class="segmented" role="group" aria-label="Изглед">
					<button
						aria-pressed={view.kind === 'class'}
						onclick={() => changeView({ kind: 'class', index: 0 })}>Паралелки</button
					>
					<button
						aria-pressed={view.kind === 'teacher'}
						onclick={() => changeView({ kind: 'teacher', index: 0 })}
						data-testid="view-teachers">Учители</button
					>
				</div>
				<select
					aria-label={view.kind === 'class' ? 'Паралелка' : 'Учител'}
					data-testid="timetable-pick"
					value={view.index}
					onchange={(e) => changeView({ ...view, index: Number(e.currentTarget.value) })}
				>
					{#each view.kind === 'class' ? school.classes : school.teachers as entity, i (i)}
						<option value={i}>{entity.name}</option>
					{/each}
				</select>
			</div>
			<TimetableGrid
				{cells}
				days={school.days}
				kind={view.kind}
				{pins}
				edit={editing ? { selected, pins, moved, preview, onhour: pick, oncell: put } : null}
			/>
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
	.rules,
	.conflicts {
		margin: var(--s-3) 0;
		padding-left: var(--s-5);
	}
	.conflicts {
		font-size: var(--text-sm);
	}
	.actions,
	.pick,
	.toolbar {
		display: flex;
		flex-wrap: wrap;
		gap: var(--s-3);
		align-items: center;
	}
	.pick {
		margin-bottom: var(--s-3);
	}
	.toolbar {
		margin-bottom: var(--s-2);
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
