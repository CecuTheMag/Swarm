<script lang="ts">
	// The schools a network keeps for its timetables (spec 0014): start one empty or from
	// the plan a deputy director already has in a spreadsheet.
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { api, failureText } from '#lib/api.ts';
	import { can, useNetwork } from '#lib/console/networks.svelte.ts';
	import { emptySchool } from '#lib/school/edit.ts';
	import { importFile, PLAN_HEADERS } from '#lib/school/import.ts';
	import ConfirmButton from '#lib/ui/ConfirmButton.svelte';
	import type { SchoolRecord } from '#lib/protocol/SchoolRecord.ts';
	import type { SchoolSummary } from '#lib/protocol/SchoolSummary.ts';
	import type { TtSchool } from '#lib/protocol/TtSchool.ts';

	const network = useNetwork();
	const id = $derived(page.params.id!);
	const admin = $derived(network.view ? can(network.view.role, 'admin') : false);

	let schools = $state<SchoolSummary[]>([]);
	let loaded = $state(false);
	let error = $state<string | null>(null);
	/** Why a picked file could not be imported, one line per row. */
	let importErrors = $state<string[]>([]);
	let busy = $state(false);

	async function load() {
		try {
			schools = await api<SchoolSummary[]>(`/api/networks/${id}/schools`);
		} catch (e) {
			error = failureText(e);
		} finally {
			loaded = true;
		}
	}
	$effect(() => {
		void load();
	});

	/**
	 * A new school is saved empty only in the editor: the coordinator keeps schools a
	 * timetable can be searched for, so the first save needs one lesson. Until then it
	 * lives in the editor, as a draft.
	 */
	function startEmpty() {
		try {
			sessionStorage.setItem(`swarm-new-school-${id}`, JSON.stringify(emptySchool('Ново училище')));
		} catch {
			error = 'Браузърът не позволява чернова: внесете училище от файл.';
			return;
		}
		void goto(resolve('/app/n/[id]/schools/[school]', { id, school: 'new' }));
	}

	async function create(school: TtSchool) {
		const created = await api<SchoolRecord>(`/api/networks/${id}/schools`, { body: { school } });
		await goto(resolve('/app/n/[id]/schools/[school]', { id, school: created.summary.id }));
	}

	async function pick(event: Event) {
		const input = event.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		if (!file) return;
		busy = true;
		error = null;
		importErrors = [];
		try {
			const imported = importFile(file.name, await file.text());
			if ('errors' in imported) {
				importErrors = imported.errors;
				return;
			}
			await create(imported.school);
		} catch (e) {
			error = failureText(e);
		} finally {
			busy = false;
			input.value = '';
		}
	}

	async function remove(school: SchoolSummary) {
		error = null;
		try {
			await api(`/api/networks/${id}/schools/${school.id}`, { method: 'DELETE' });
			await load();
		} catch (e) {
			error = failureText(e);
		}
	}
</script>

<svelte:head>
	<title>Училища · {network.view?.name} · Рояк</title>
</svelte:head>

{#if error}<p class="notice bad" role="alert" data-testid="school-error">{error}</p>{/if}

<section class="card">
	<h2>Училища</h2>
	<p class="muted">
		Данните за седмичното разписание: паралелки, учители, стаи и учебен план. Пазят се тук, за да
		търсите разписание отново, когато нещо се промени, и да не ги въвеждате пак.
	</p>
	{#if loaded && schools.length === 0}
		<p class="muted" data-testid="no-schools">
			Още няма училища{admin ? '. Започнете от плана си в таблица или от празно.' : '.'}
		</p>
	{:else}
		<ul class="schools">
			{#each schools as s (s.id)}
				<li data-testid="school" data-id={s.id}>
					<a href={resolve('/app/n/[id]/schools/[school]', { id, school: s.id })}>
						<strong>{s.name}</strong>
						<span class="muted small">
							{s.classes} паралелки · {s.teachers} учители · {s.hours} часа седмично
						</span>
						<span class="muted small">
							Запазено {new Date(s.updated_at).toLocaleString('bg-BG')}{s.updated_by
								? ` от ${s.updated_by}`
								: ''}
						</span>
					</a>
					{#if admin}
						<ConfirmButton
							label="Изтрий"
							confirm="Изтрий завинаги?"
							testid="delete-school"
							onconfirm={() => remove(s)}
						/>
					{/if}
				</li>
			{/each}
		</ul>
	{/if}
</section>

{#if admin}
	<section class="card">
		<h2>Ново училище</h2>
		<div class="start">
			<label class="button file" class:disabled={busy}>
				Внеси от таблица (CSV) или JSON
				<input
					type="file"
					accept=".csv,.json,text/csv,application/json"
					disabled={busy}
					data-testid="school-file"
					onchange={pick}
				/>
			</label>
			<button type="button" class="secondary" onclick={startEmpty} data-testid="new-school"
				>Започни от празно</button
			>
		</div>
		{#if importErrors.length > 0}
			<div class="notice bad" role="alert" data-testid="import-errors">
				<p>Файлът не е внесен. Поправете тези редове и опитайте пак:</p>
				<ul>
					{#each importErrors.slice(0, 20) as e, i (i)}<li>{e}</li>{/each}
				</ul>
				{#if importErrors.length > 20}<p>…и още {importErrors.length - 20}.</p>{/if}
			</div>
		{/if}
		<p class="muted small">
			Таблицата е по един ред за всеки предмет на всяка паралелка, с колони
			{PLAN_HEADERS.slice(0, 4)
				.map((h) => `„${h}“`)
				.join(', ')} и по желание „{PLAN_HEADERS[4]}“ и „{PLAN_HEADERS[5]}“ (стая, която не бива да
			е заета два пъти: лаборатория, физкултурен салон). Запазете я от Excel или LibreOffice като
			CSV. Седмицата е 5 дни по 7 часа; сменя се в училището след внасянето.
		</p>
	</section>
{/if}

<style>
	section + section {
		margin-top: var(--s-4);
	}
	h2 {
		font-size: var(--text-lg);
		margin: 0 0 var(--s-3);
	}
	.schools {
		list-style: none;
		margin: var(--s-3) 0 0;
		padding: 0;
	}
	.schools li {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: var(--s-3);
		padding: var(--s-3) 0;
		border-top: 1px solid var(--border);
	}
	.schools a {
		display: grid;
		gap: 2px;
		color: inherit;
		text-decoration: none;
		min-width: 0;
	}
	.schools a:hover strong {
		text-decoration: underline;
	}
	.start {
		display: flex;
		flex-wrap: wrap;
		gap: var(--s-3);
		margin-bottom: var(--s-3);
	}
	.file {
		position: relative;
		cursor: pointer;
	}
	.file input {
		position: absolute;
		inset: 0;
		opacity: 0;
		cursor: pointer;
	}
	.file.disabled {
		opacity: 0.6;
		pointer-events: none;
	}
	.notice ul {
		margin: var(--s-2) 0 0;
		padding-left: 1.2em;
	}
	.notice p {
		margin: 0;
	}
	.small {
		font-size: var(--text-sm);
	}
</style>
