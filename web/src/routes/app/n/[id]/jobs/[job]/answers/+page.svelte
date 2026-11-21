<script lang="ts">
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { api, failureText } from '#lib/api.ts';
	import { can, useNetwork } from '#lib/console/networks.svelte.ts';
	import { answersCsv, answersText, saveFile } from '#lib/console/ai.ts';
	import ConfirmButton from '#lib/ui/ConfirmButton.svelte';
	import Icon from '#lib/ui/Icon.svelte';
	import { aiCatalogText } from '#lib/templates.ts';
	import type { AiCatalog } from '#lib/protocol/AiCatalog.ts';
	import type { AiJobParams } from '#lib/protocol/AiJobParams.ts';
	import type { JobOutputs } from '#lib/protocol/JobOutputs.ts';
	import type { JobSummary } from '#lib/protocol/JobSummary.ts';

	// A language-model job (spec 0017): each question with its answer, filling in as the
	// network's devices answer them.
	const network = useNetwork();
	const id = $derived(page.params.id!);
	const jobId = $derived(page.params.job!);
	const admin = $derived(network.view ? can(network.view.role, 'admin') : false);

	let job = $state<JobSummary | null>(null);
	let answers = $state<(string | null)[]>([]);
	let error = $state<string | null>(null);
	let missing = $state(false);

	const base = $derived(`/api/networks/${id}/jobs/${jobId}`);
	async function load() {
		try {
			const [j, o] = await Promise.all([api<JobSummary>(base), api<JobOutputs>(`${base}/outputs`)]);
			job = j;
			answers = o.outputs;
			error = null;
		} catch (e) {
			if (failureText(e) === 'not found') missing = true;
			else error = failureText(e);
		}
	}
	// Every two seconds while it runs; a finished job no longer changes.
	$effect(() => {
		void load();
		const timer = setInterval(() => {
			if (!job || job.status === 'queued' || job.status === 'running') void load();
		}, 2000);
		return () => clearInterval(timer);
	});

	const params = $derived(job?.params as AiJobParams | undefined);
	const prompts = $derived(params?.prompts ?? []);
	const model = $derived(
		(JSON.parse(aiCatalogText) as AiCatalog).models.find((m) => m.id === params?.model)
	);
	const answered = $derived(answers.filter((a) => a !== null).length);
	const running = $derived(job?.status === 'queued' || job?.status === 'running');
	const statusText: Record<string, string> = {
		queued: 'чака устройства',
		running: 'отговаря се',
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
	const csv = () =>
		saveFile(`swarm-${jobId}.csv`, 'text/csv;charset=utf-8', answersCsv(prompts, answers));
	const text = () =>
		saveFile(`swarm-${jobId}.txt`, 'text/plain;charset=utf-8', answersText(prompts, answers));
</script>

<svelte:head>
	<title>Текст с ИИ · {network.view?.name} · Рояк</title>
</svelte:head>

<a class="back" href={resolve('/app/n/[id]/jobs', { id })}><Icon name="arrow" size={14} /> Задачи</a
>

{#if missing}
	<div class="card">
		<h2>Задачата не е намерена</h2>
		<p class="muted">Може да е изтрита или да е от друга мрежа.</p>
	</div>
{:else if job && params}
	<section class="card head" data-testid="answers" data-status={job.status}>
		<div class="title">
			<span class="icon"><Icon name="chip" size={22} /></span>
			<div>
				<h2>Текст с ИИ · {model?.name ?? params.model}</h2>
				<p class="muted">
					{answered} от {prompts.length} отговора{params.system
						? ` · указания: „${params.system}“`
						: ''}
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
			<div style:width="{(100 * answered) / Math.max(prompts.length, 1)}%"></div>
		</div>
		<div class="actions">
			{#if answered > 0}
				<button class="secondary" onclick={csv} data-testid="download-csv"
					>Изтегли като таблица</button
				>
				<button class="secondary" onclick={text}>Изтегли като текст</button>
			{/if}
			{#if admin && running}
				<ConfirmButton label="Спри" confirm="Спри задачата?" onconfirm={stop} />
			{/if}
		</div>
		{#if job.error && job.error !== 'cancelled'}<p class="notice bad">{job.error}</p>{/if}
	</section>

	{#if error}<p class="notice bad" role="alert">{error}</p>{/if}

	<section class="card" aria-live="polite">
		{#if running && answered === 0}
			<p class="muted">
				Устройствата зареждат модела. Първото зареждане изтегля {model
					? Math.round(model.files.reduce((s, f) => s + f.size, 0) / 1e6)
					: '…'} MB; следващите четат копието на устройството.
			</p>
		{/if}
		<ol class="qa">
			{#each prompts as prompt, i (i)}
				<li data-testid="answer" data-answered={answers[i] !== null && answers[i] !== undefined}>
					<p class="q">{prompt}</p>
					{#if answers[i] !== null && answers[i] !== undefined}
						<p class="a">{answers[i]}</p>
					{:else}
						<p class="a muted">{running ? 'чака отговор…' : '—'}</p>
					{/if}
				</li>
			{/each}
		</ol>
	</section>
{/if}

<style>
	section + section {
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
	h2 {
		margin: 0;
		font-size: var(--text-lg);
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
		overflow-wrap: anywhere;
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
	.qa {
		margin: 0;
		padding-left: 1.6em;
		display: grid;
		gap: var(--s-3);
	}
	.qa li {
		padding-left: var(--s-1);
	}
	.q {
		margin: 0;
		font-weight: 600;
		overflow-wrap: anywhere;
	}
	.a {
		margin: 4px 0 0;
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}
</style>
