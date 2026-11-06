<script lang="ts">
	// A school's data, as a deputy director keeps it (spec 0014): the week, teachers,
	// classes, rooms and the plan of lessons; what makes it impossible, said before a
	// search; and the search itself.
	import { beforeNavigate, goto } from '$app/navigation';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { api, ApiFailure, failureText } from '#lib/api.ts';
	import { can, useNetwork } from '#lib/console/networks.svelte.ts';
	import { check } from '#lib/school/checks.ts';
	import {
		add,
		addLesson,
		inputOf,
		remove,
		removeLesson,
		rename,
		setWeek,
		toggleUnavailable,
		updateLesson,
		type Kind,
		type LessonInput
	} from '#lib/school/edit.ts';
	import { defaultMaxPerDay, exportPlan } from '#lib/school/import.ts';
	import WeekPicker from '#lib/school/WeekPicker.svelte';
	import ConfirmButton from '#lib/ui/ConfirmButton.svelte';
	import type { JobSummary } from '#lib/protocol/JobSummary.ts';
	import type { SchoolRecord } from '#lib/protocol/SchoolRecord.ts';
	import type { TtSchool } from '#lib/protocol/TtSchool.ts';
	import type { TtSlot } from '#lib/protocol/TtSlot.ts';

	const network = useNetwork();
	const id = $derived(page.params.id!);
	const schoolId = $derived(page.params.school!);
	const admin = $derived(network.view ? can(network.view.role, 'admin') : false);
	const draftKey = $derived(`swarm-new-school-${id}`);

	let school = $state<TtSchool | null>(null);
	/** The version this edit started from; null for a school not saved yet. */
	let version = $state<number | null>(null);
	/** The school as last saved, to tell whether anything changed. */
	let saved = $state('');
	let missing = $state(false);
	let error = $state<string | null>(null);
	let notice = $state<string | null>(null);
	let busy = $state(false);

	const dirty = $derived(school !== null && JSON.stringify(school) !== saved);
	const findings = $derived(school ? check(school) : []);
	const problems = $derived(findings.filter((f) => f.level === 'problem'));
	const warnings = $derived(findings.filter((f) => f.level === 'warning'));

	async function load() {
		error = null;
		if (schoolId === 'new') {
			let draft: TtSchool | null;
			try {
				draft = JSON.parse(sessionStorage.getItem(draftKey) ?? 'null') as TtSchool | null;
			} catch {
				draft = null;
			}
			school = draft;
			version = null;
			saved = '';
			missing = draft === null;
			return;
		}
		try {
			const record = await api<SchoolRecord>(`/api/networks/${id}/schools/${schoolId}`);
			school = record.school;
			// One class at a time, as a plan is read; „всички“ is one choice away.
			classFilter = record.school.classes.length > 0 ? 0 : 'all';
			form = blank();
			version = record.summary.version;
			saved = JSON.stringify(record.school);
		} catch (e) {
			if (e instanceof ApiFailure && e.status === 404) missing = true;
			else error = failureText(e);
		}
	}
	$effect(() => {
		void load();
	});

	// A draft lives in this tab until its first save.
	$effect(() => {
		if (schoolId === 'new' && school) {
			try {
				sessionStorage.setItem(draftKey, JSON.stringify(school));
			} catch {
				// Private browsing without storage: the draft lasts as long as the page.
			}
		}
	});

	beforeNavigate((nav) => {
		if (
			dirty &&
			!busy &&
			nav.type !== 'leave' &&
			!confirm('Има незапазени промени. Да се изоставят ли?')
		) {
			nav.cancel();
		}
	});
	function beforeUnload(event: BeforeUnloadEvent) {
		if (dirty) event.preventDefault();
	}

	/** Runs an edit; a refused one is said, and changes nothing. */
	function edit(change: () => void): boolean {
		error = null;
		notice = null;
		try {
			change();
			return true;
		} catch (e) {
			error = failureText(e);
			return false;
		}
	}

	async function save() {
		if (!school) return;
		busy = true;
		error = null;
		notice = null;
		try {
			const body = { school: $state.snapshot(school), ...(version === null ? {} : { version }) };
			const record =
				version === null
					? await api<SchoolRecord>(`/api/networks/${id}/schools`, { body })
					: await api<SchoolRecord>(`/api/networks/${id}/schools/${schoolId}`, {
							method: 'PUT',
							body
						});
			// The coordinator's copy from now on: what it stored, in the order it writes it.
			school = record.school;
			version = record.summary.version;
			saved = JSON.stringify(record.school);
			notice = 'Запазено.';
			if (schoolId === 'new') {
				try {
					sessionStorage.removeItem(draftKey);
				} catch {
					// Nothing to clear.
				}
				await goto(resolve('/app/n/[id]/schools/[school]', { id, school: record.summary.id }), {
					replaceState: true
				});
			}
		} catch (e) {
			error =
				e instanceof ApiFailure && e.status === 409
					? `${e.message}. Отворете училището отново, за да видите промените; вашите не са запазени.`
					: failureText(e);
		} finally {
			busy = false;
		}
	}

	function download(name: string, type: string, text: string) {
		const a = document.createElement('a');
		a.href = URL.createObjectURL(new Blob([text], { type }));
		a.download = name;
		a.click();
		URL.revokeObjectURL(a.href);
	}

	// The week.
	function week(days: number, periods: number) {
		edit(() => setWeek(school!, days, periods));
	}

	// Teachers, classes and rooms.
	type Part = 'plan' | 'teacher' | 'class' | 'room';
	let part = $state<Part>('plan');
	/** Whose week is open for marking the hours they are away. */
	let picking = $state<{ kind: 'class' | 'teacher'; index: number } | null>(null);
	let newName = $state('');

	const hoursOf = (kind: Kind, index: number) =>
		school!.lessons
			.filter((l) => (kind === 'room' ? l.room === index : l[kind] === index))
			.reduce((n, l) => n + l.per_week, 0);

	function addEntry(event: SubmitEvent, kind: Kind) {
		event.preventDefault();
		if (edit(() => add(school!, kind, newName))) newName = '';
	}

	function renameEntry(kind: Kind, index: number, input: HTMLInputElement) {
		if (!edit(() => rename(school!, kind, index, input.value))) {
			input.value =
				school![kind === 'class' ? 'classes' : kind === 'teacher' ? 'teachers' : 'rooms'][
					index
				].name;
		}
	}

	function removeEntry(kind: Kind, index: number) {
		if (edit(() => remove(school!, kind, index))) {
			if (picking && picking.kind === kind) picking = null;
		}
	}

	function toggle(kind: 'class' | 'teacher', index: number, slots: TtSlot[]) {
		edit(() => {
			for (const s of slots) toggleUnavailable(school!, kind, index, s);
		});
	}

	function maxPerDay(index: number, value: string) {
		edit(() => {
			const t = school!.teachers[index];
			const n = Number(value);
			if (value.trim() === '') delete t.max_per_day;
			else if (!Number.isInteger(n) || n < 1 || n > school!.periods) {
				throw new Error(`Най-много на ден е от 1 до ${school!.periods}, или празно.`);
			} else t.max_per_day = n;
		});
	}

	// The plan.
	let classFilter = $state<number | 'all'>('all');
	/** The lesson being changed, or null while adding one. */
	let editing = $state<number | null>(null);
	const blank = (): LessonInput => ({
		class: classFilter === 'all' ? '' : (school?.classes[classFilter]?.name ?? ''),
		subject: '',
		teacher: '',
		per_week: 2,
		max_per_day: 1,
		room: ''
	});
	let form = $state<LessonInput>({
		class: '',
		subject: '',
		teacher: '',
		per_week: 2,
		max_per_day: 1,
		room: ''
	});
	const shown = $derived(
		school
			? school.lessons
					.map((l, index) => ({ l, index }))
					.filter(({ l }) => classFilter === 'all' || l.class === classFilter)
			: []
	);

	function submitLesson(event: SubmitEvent) {
		event.preventDefault();
		const input = {
			...form,
			per_week: Number(form.per_week),
			max_per_day: Number(form.max_per_day)
		};
		const ok =
			editing === null
				? edit(() => addLesson(school!, input))
				: edit(() => updateLesson(school!, editing!, input));
		if (ok) {
			editing = null;
			form = { ...blank(), class: input.class };
		}
	}

	function startEditing(index: number) {
		editing = index;
		form = inputOf(school!, index);
	}

	function perWeekChanged() {
		// The usual spread over the week, until someone sets it.
		if (editing === null) form.max_per_day = defaultMaxPerDay(Number(form.per_week), school!.days);
	}

	// The search.
	let effort = $state<'quick' | 'normal' | 'deep'>('normal');
	let searches = $state(32);
	let quorum = $state(2);
	const ITERATIONS = { quick: 500_000, normal: 2_000_000, deep: 8_000_000 };

	async function search() {
		if (!school) return;
		busy = true;
		error = null;
		try {
			const seed = crypto.getRandomValues(new Uint32Array(1))[0];
			await api<JobSummary>(`/api/networks/${id}/jobs`, {
				body: {
					template: 'tt',
					params: {
						school: $state.snapshot(school),
						searches,
						iterations: ITERATIONS[effort],
						seed
					},
					...(network.view?.public ? {} : { quorum })
				}
			});
			await goto(resolve('/app/n/[id]/jobs', { id }));
		} catch (e) {
			error = failureText(e);
		} finally {
			busy = false;
		}
	}
</script>

<svelte:window onbeforeunload={beforeUnload} />

<svelte:head>
	<title>{school?.name ?? 'Училище'} · {network.view?.name} · Рояк</title>
</svelte:head>

<p><a href={resolve('/app/n/[id]/schools', { id })}>← Всички училища</a></p>

{#if missing}
	<div class="card">
		<h2>Училището не е намерено</h2>
		<p class="muted">Може да е изтрито.</p>
	</div>
{:else if school}
	<header class="card head">
		<label class="name"
			>Име
			<input
				value={school.name}
				maxlength="80"
				disabled={!admin}
				data-testid="school-name"
				onchange={(e) => {
					const v = e.currentTarget.value.trim();
					if (v) school!.name = v;
					else e.currentTarget.value = school!.name;
				}}
			/></label
		>
		<div class="actions">
			<span class="badge" class:ok={!dirty} data-testid="save-state" data-dirty={dirty}
				>{dirty ? 'незапазени промени' : 'запазено'}</span
			>
			{#if admin}
				<button type="button" disabled={!dirty || busy} onclick={save} data-testid="save-school"
					>Запази</button
				>
			{/if}
			<button
				type="button"
				class="secondary"
				disabled={school.lessons.length === 0}
				onclick={() => download(`${school!.name}.csv`, 'text/csv', exportPlan(school!))}
				data-testid="export-csv">CSV</button
			>
			<button
				type="button"
				class="secondary"
				onclick={() =>
					download(`${school!.name}.json`, 'application/json', JSON.stringify(school, null, 1))}
				data-testid="export-json">JSON</button
			>
		</div>
	</header>

	{#if error}<p class="notice bad" role="alert" data-testid="school-error">{error}</p>{/if}
	{#if notice}<p class="notice ok" role="status" data-testid="school-notice">{notice}</p>{/if}

	<section class="card">
		<h2>Седмица</h2>
		<div class="fields">
			<label
				>Учебни дни
				<select
					value={school.days}
					disabled={!admin}
					data-testid="school-days"
					onchange={(e) => week(Number(e.currentTarget.value), school!.periods)}
				>
					{#each [5, 6] as d (d)}<option value={d}>{d}</option>{/each}
				</select></label
			>
			<label
				>Часа на ден
				<input
					type="number"
					min="1"
					max="12"
					value={school.periods}
					disabled={!admin}
					data-testid="school-periods"
					onchange={(e) => week(school!.days, Number(e.currentTarget.value))}
				/></label
			>
			<p class="muted small">
				{school.classes.length} паралелки · {school.teachers.length} учители · {school.rooms.length}
				стаи · {school.lessons.reduce((n, l) => n + l.per_week, 0)} часа седмично
			</p>
		</div>
	</section>

	<section class="card" data-testid="findings">
		<h2>Проверка</h2>
		{#if school.lessons.length === 0}
			<p class="muted">Добавете часовете от учебния план.</p>
		{:else if findings.length === 0}
			<p class="notice ok" data-testid="no-findings">
				Нищо не пречи на разписанието. Дали има такова без конфликти, ще покаже търсенето.
			</p>
		{:else}
			{#each problems as f, i (i)}
				<p class="notice bad" data-testid="problem">{f.text}</p>
			{/each}
			{#each warnings as f, i (i)}
				<p class="notice warn" data-testid="warning">{f.text}</p>
			{/each}
		{/if}
	</section>

	<section class="card">
		<div class="segmented parts" role="group" aria-label="Част от данните">
			<button
				type="button"
				aria-pressed={part === 'plan'}
				onclick={() => (part = 'plan')}
				data-testid="part-plan">Учебен план</button
			>
			<button
				type="button"
				aria-pressed={part === 'teacher'}
				onclick={() => (part = 'teacher')}
				data-testid="part-teachers">Учители</button
			>
			<button
				type="button"
				aria-pressed={part === 'class'}
				onclick={() => (part = 'class')}
				data-testid="part-classes">Паралелки</button
			>
			<button
				type="button"
				aria-pressed={part === 'room'}
				onclick={() => (part = 'room')}
				data-testid="part-rooms">Стаи</button
			>
		</div>

		{#if part === 'plan'}
			<label class="filter"
				>Паралелка
				<select
					bind:value={classFilter}
					data-testid="class-filter"
					onchange={() => {
						if (editing === null) form = blank();
					}}
				>
					<option value="all">всички</option>
					{#each school.classes as c, i (i)}<option value={i}>{c.name}</option>{/each}
				</select></label
			>
			<div class="table-wrap">
				<table data-testid="plan">
					<thead>
						<tr>
							<th scope="col">Паралелка</th>
							<th scope="col">Предмет</th>
							<th scope="col">Учител</th>
							<th scope="col" class="num">Седмично</th>
							<th scope="col" class="num">На ден</th>
							<th scope="col">Стая</th>
							{#if admin}<th scope="col"><span class="sr-only">Действия</span></th>{/if}
						</tr>
					</thead>
					<tbody>
						{#each shown as { l, index } (index)}
							<tr data-testid="lesson" class:editing={editing === index}>
								<td>{school.classes[l.class].name}</td>
								<td>{l.subject}</td>
								<td>{school.teachers[l.teacher].name}</td>
								<td class="num">{l.per_week}</td>
								<td class="num">{l.max_per_day}</td>
								<td>{l.room == null ? '' : school.rooms[l.room].name}</td>
								{#if admin}
									<td class="row-actions">
										<button type="button" class="ghost sm" onclick={() => startEditing(index)}
											>Промени</button
										>
										<button
											type="button"
											class="ghost sm"
											data-testid="remove-lesson"
											onclick={() => {
												edit(() => removeLesson(school!, index));
												editing = null;
											}}>Махни</button
										>
									</td>
								{/if}
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
			{#if admin}
				<form class="lesson-form" onsubmit={submitLesson} data-testid="lesson-form">
					<label
						>Паралелка <input
							bind:value={form.class}
							list="class-names"
							required
							maxlength="80"
							data-testid="lesson-class"
						/></label
					>
					<label
						>Предмет <input
							bind:value={form.subject}
							list="subject-names"
							required
							maxlength="80"
							data-testid="lesson-subject"
						/></label
					>
					<label
						>Учител <input
							bind:value={form.teacher}
							list="teacher-names"
							required
							maxlength="80"
							data-testid="lesson-teacher"
						/></label
					>
					<label
						>Седмично <input
							type="number"
							min="1"
							max={school.days * school.periods}
							bind:value={form.per_week}
							oninput={perWeekChanged}
							required
							data-testid="lesson-per-week"
						/></label
					>
					<label
						>Най-много на ден <input
							type="number"
							min="1"
							max={school.periods}
							bind:value={form.max_per_day}
							required
							data-testid="lesson-max"
						/></label
					>
					<label
						>Стая (ако е нужна) <input
							bind:value={form.room}
							list="room-names"
							maxlength="80"
							data-testid="lesson-room"
						/></label
					>
					<div class="form-actions">
						<button type="submit" data-testid="lesson-submit"
							>{editing === null ? 'Добави час' : 'Запази часа'}</button
						>
						{#if editing !== null}
							<button
								type="button"
								class="secondary"
								onclick={() => {
									editing = null;
									form = blank();
								}}>Отказ</button
							>
						{/if}
					</div>
				</form>
				<datalist id="class-names">
					{#each school.classes as c, i (i)}<option value={c.name}></option>{/each}
				</datalist>
				<datalist id="teacher-names">
					{#each school.teachers as t, i (i)}<option value={t.name}></option>{/each}
				</datalist>
				<datalist id="room-names">
					{#each school.rooms as r, i (i)}<option value={r.name}></option>{/each}
				</datalist>
				<datalist id="subject-names">
					{#each [...new Set(school.lessons.map((l) => l.subject))] as s (s)}<option value={s}
						></option>{/each}
				</datalist>
			{/if}
		{:else}
			{@const kind = part}
			{@const list =
				kind === 'class' ? school.classes : kind === 'teacher' ? school.teachers : school.rooms}
			<ul class="entries">
				{#each list as entry, index (index)}
					<li data-testid="entry">
						<div class="entry-row">
							<input
								class="entry-name"
								value={entry.name}
								maxlength="80"
								disabled={!admin}
								aria-label="Име"
								data-testid="entry-name"
								onchange={(e) => renameEntry(kind, index, e.currentTarget)}
							/>
							<span class="muted small">{hoursOf(kind, index)} часа седмично</span>
							{#if kind === 'teacher'}
								<label class="inline"
									>най-много на ден
									<input
										type="number"
										min="1"
										max={school.periods}
										value={school.teachers[index].max_per_day ?? ''}
										disabled={!admin}
										data-testid="teacher-max"
										onchange={(e) => maxPerDay(index, e.currentTarget.value)}
									/></label
								>
							{/if}
							{#if kind !== 'room'}
								{@const away = (kind === 'class' ? school.classes : school.teachers)[index]
									.unavailable}
								<button
									type="button"
									class="secondary sm"
									aria-expanded={picking?.kind === kind && picking.index === index}
									data-testid="pick-away"
									onclick={() =>
										(picking =
											picking?.kind === kind && picking.index === index ? null : { kind, index })}
									>{away.length === 0 ? 'Отсъствия' : `Отсъствия: ${away.length} часа`}</button
								>
							{/if}
							{#if admin}
								<button
									type="button"
									class="ghost sm"
									data-testid="remove-entry"
									onclick={() => removeEntry(kind, index)}>Махни</button
								>
							{/if}
						</div>
						{#if kind !== 'room' && picking?.kind === kind && picking.index === index}
							<WeekPicker
								days={school.days}
								periods={school.periods}
								away={(kind === 'class' ? school.classes : school.teachers)[index].unavailable}
								label="Часове, в които {entry.name} отсъства"
								ontoggle={(slots) => (admin ? toggle(kind, index, slots) : undefined)}
							/>
						{/if}
					</li>
				{/each}
			</ul>
			{#if list.length === 0}<p class="muted">Още няма.</p>{/if}
			{#if admin}
				<form class="add-entry" onsubmit={(e) => addEntry(e, kind)}>
					<input
						bind:value={newName}
						maxlength="80"
						required
						placeholder={kind === 'class'
							? '10В'
							: kind === 'teacher'
								? 'Име и фамилия'
								: 'Лаборатория'}
						aria-label="Ново име"
						data-testid="new-entry"
					/>
					<button type="submit" class="secondary">Добави</button>
				</form>
			{/if}
		{/if}
	</section>

	{#if admin}
		<section class="card">
			<h2>Търсене на разписание</h2>
			<div class="segmented" role="group" aria-label="Колко да се търси">
				<button type="button" aria-pressed={effort === 'quick'} onclick={() => (effort = 'quick')}
					>Бързо</button
				>
				<button type="button" aria-pressed={effort === 'normal'} onclick={() => (effort = 'normal')}
					>Нормално</button
				>
				<button type="button" aria-pressed={effort === 'deep'} onclick={() => (effort = 'deep')}
					>Задълбочено</button
				>
			</div>
			<div class="fields">
				<label
					>Търсения <input
						type="number"
						min="1"
						max="500"
						bind:value={searches}
						data-testid="school-searches"
					/></label
				>
				{#if !network.view?.public}
					<label
						>Проверка
						<select bind:value={quorum} data-testid="school-quorum">
							<option value={1}>1 резултат (вярвам на устройствата)</option>
							<option value={2}>2 съвпадащи резултата</option>
							<option value={3}>3 съвпадащи резултата</option>
						</select></label
					>
				{/if}
			</div>
			{#if dirty}
				<p class="muted small">Търси се в училището, както е запазено: първо запазете промените.</p>
			{:else if problems.length > 0}
				<p class="muted small">Първо поправете проблемите отгоре.</p>
			{/if}
			<button
				type="button"
				disabled={busy || dirty || version === null || problems.length > 0}
				onclick={search}
				data-testid="school-search">Търси разписание</button
			>
		</section>
		{#if version !== null}
			<section class="card danger-zone">
				<ConfirmButton
					label="Изтрий училището"
					confirm="Изтрий завинаги?"
					testid="delete-school"
					onconfirm={async () => {
						try {
							await api(`/api/networks/${id}/schools/${schoolId}`, { method: 'DELETE' });
							saved = JSON.stringify(school);
							await goto(resolve('/app/n/[id]/schools', { id }));
						} catch (e) {
							error = failureText(e);
						}
					}}
				/>
				<span class="muted small">Задачите, търсили разписание за него, остават.</span>
			</section>
		{/if}
	{/if}
{/if}

<style>
	section,
	.head {
		margin-top: var(--s-4);
	}
	h2 {
		font-size: var(--text-lg);
		margin: 0 0 var(--s-3);
	}
	.head {
		display: flex;
		flex-wrap: wrap;
		justify-content: space-between;
		align-items: end;
		gap: var(--s-3);
	}
	.name {
		flex: 1 1 280px;
	}
	.actions,
	.form-actions,
	.entry-row,
	.add-entry {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--s-2);
	}
	.fields {
		display: flex;
		flex-wrap: wrap;
		align-items: end;
		gap: var(--s-3);
	}
	.fields label {
		width: 10rem;
	}
	.parts {
		margin-bottom: var(--s-4);
	}
	.filter {
		max-width: 14rem;
		margin-bottom: var(--s-3);
	}
	th.num,
	td.num {
		width: 5.5rem;
		text-align: right;
	}
	tr.editing td {
		background: var(--brand-soft);
	}
	.row-actions {
		white-space: nowrap;
		text-align: right;
	}
	.lesson-form {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
		gap: var(--s-3);
		align-items: end;
		margin-top: var(--s-4);
	}
	.entries {
		list-style: none;
		margin: 0;
		padding: 0;
	}
	.entries li {
		display: grid;
		gap: var(--s-3);
		padding: var(--s-2) 0;
		border-top: 1px solid var(--border);
	}
	.entry-name {
		flex: 0 1 18rem;
		min-width: 10rem;
	}
	.inline {
		display: flex;
		align-items: center;
		gap: var(--s-2);
		font-size: var(--text-sm);
	}
	.inline input {
		width: 4.5rem;
	}
	.add-entry {
		margin-top: var(--s-3);
	}
	.add-entry input {
		max-width: 22rem;
	}
	.danger-zone {
		display: flex;
		align-items: center;
		gap: var(--s-3);
	}
	.notice + .notice {
		margin-top: var(--s-2);
	}
	.small {
		font-size: var(--text-sm);
	}
	.sr-only {
		position: absolute;
		width: 1px;
		height: 1px;
		overflow: hidden;
		clip: rect(0 0 0 0);
	}
</style>
