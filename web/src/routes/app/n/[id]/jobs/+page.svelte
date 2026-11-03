<script lang="ts">
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { LiveDashboard } from '#lib/dashboard/live.svelte.ts';
	import { sortedJobs } from '#lib/dashboard/state.ts';
	import { api, failureText } from '#lib/api.ts';
	import { can, useNetwork } from '#lib/console/networks.svelte.ts';
	import ConfirmButton from '#lib/ui/ConfirmButton.svelte';
	import Icon from '#lib/ui/Icon.svelte';
	import type { JobStatus } from '#lib/protocol/JobStatus.ts';
	import type { JobSummary } from '#lib/protocol/JobSummary.ts';
	import type { PiResult } from '#lib/protocol/PiResult.ts';
	import type { PtResult } from '#lib/protocol/PtResult.ts';

	const network = useNetwork();
	const id = $derived(page.params.id!);
	const admin = $derived(network.view ? can(network.view.role, 'admin') : false);

	let live = $state<LiveDashboard | null>(null);
	$effect(() => {
		const l = new LiveDashboard(undefined, `/ws/networks/${id}/live`);
		l.start();
		live = l;
		return () => l.stop();
	});
	const jobs = $derived(live ? sortedJobs(live.state) : []);

	// What to compute, in plain terms; the technical fields stay one click away.
	let template = $state<'pt' | 'pi'>('pt');
	let quality = $state<'fast' | 'normal' | 'high'>('normal');
	const QUALITY = {
		fast: { width: 960, height: 540, spp: 64, passes: 4 },
		normal: { width: 1280, height: 720, spp: 128, passes: 8 },
		high: { width: 1920, height: 1080, spp: 128, passes: 16 }
	};
	let advanced = $state(false);
	let width = $state(1280);
	let height = $state(720);
	let tile = $state(64);
	let spp = $state(128);
	let passes = $state(8);
	let units = $state(64);
	let samples = $state(16_777_216);
	let seed = $state(42);
	$effect(() => {
		({ width, height, spp, passes } = QUALITY[quality]);
	});

	let error = $state<string | null>(null);
	let busy = $state(false);

	async function start(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		error = null;
		try {
			await api<JobSummary>(`/api/networks/${id}/jobs`, {
				body:
					template === 'pi'
						? { template, params: { units, samples_per_unit: samples, seed } }
						: { template, params: { width, height, tile, spp, passes, seed } }
			});
		} catch (e) {
			error = failureText(e);
		} finally {
			busy = false;
		}
	}

	async function stop(job: string) {
		error = null;
		try {
			await api(`/api/networks/${id}/jobs/${job}/cancel`, { body: {} });
		} catch (e) {
			error = failureText(e);
		}
	}

	const statusText: Record<JobStatus, string> = {
		queued: 'чака',
		running: 'изчислява се',
		done: 'готова',
		failed: 'неуспешна'
	};
	const piResult = (r: unknown) => r as PiResult;
	const ptResult = (r: unknown) => r as PtResult;
</script>

<svelte:head>
	<title>Задачи · {network.view?.name} · Рояк</title>
</svelte:head>

{#if admin}
	<section class="card new">
		<h2>Нова задача</h2>
		<form onsubmit={start}>
			<div class="segmented kinds" role="group" aria-label="Какво да се изчисли">
				<button type="button" aria-pressed={template === 'pt'} onclick={() => (template = 'pt')}>
					<Icon name="image" size={16} /> Изображение
				</button>
				<button type="button" aria-pressed={template === 'pi'} onclick={() => (template = 'pi')}>
					<Icon name="atom" size={16} /> Числото π
				</button>
			</div>

			{#if template === 'pt'}
				<p class="muted">
					Реалистично изображение чрез проследяване на лъчи. По-високото качество отнема повече
					време.
				</p>
				<div class="segmented" role="group" aria-label="Качество">
					<button type="button" aria-pressed={quality === 'fast'} onclick={() => (quality = 'fast')}
						>Бързо</button
					>
					<button
						type="button"
						aria-pressed={quality === 'normal'}
						onclick={() => (quality = 'normal')}>Нормално</button
					>
					<button type="button" aria-pressed={quality === 'high'} onclick={() => (quality = 'high')}
						>Високо</button
					>
				</div>
			{:else}
				<p class="muted">Числото π по метода Монте Карло: точен, проверим резултат.</p>
			{/if}

			<details bind:open={advanced}>
				<summary>Подробни настройки</summary>
				<div class="fields">
					{#if template === 'pi'}
						<label>Единици <input type="number" min="1" max="10000" bind:value={units} /></label>
						<label
							>Проби на единица <input
								type="number"
								min="1"
								max="1073741824"
								bind:value={samples}
							/></label
						>
					{:else}
						<label>Ширина <input type="number" min="16" max="2048" bind:value={width} /></label>
						<label>Височина <input type="number" min="16" max="2048" bind:value={height} /></label>
						<label>Плочка <input type="number" min="8" max="128" bind:value={tile} /></label>
						<label
							>Проби на проход <input type="number" min="1" max="1024" bind:value={spp} /></label
						>
						<label>Проходи <input type="number" min="1" max="256" bind:value={passes} /></label>
					{/if}
					<label>Начално число <input type="number" min="0" bind:value={seed} /></label>
				</div>
			</details>
			{#if error}<p class="notice bad" role="alert">{error}</p>{/if}
			<button type="submit" class="lg" disabled={busy} data-testid="start-job">Пусни</button>
		</form>
	</section>
{/if}

<section class="card">
	<h2>Задачи</h2>
	{#if jobs.length === 0}
		<p class="muted">Още няма задачи{admin ? '. Пуснете първата отгоре.' : '.'}</p>
	{:else}
		<ul class="jobs">
			{#each jobs as job (job.id)}
				<li class="job" data-testid="job" data-job-id={job.id} data-status={job.status}>
					<div class="main">
						<span class="kind">
							<Icon name={job.template === 'pt' ? 'image' : 'atom'} size={18} />
							{job.template === 'pt' ? 'Изображение' : 'Числото π'}
						</span>
						<span
							class="badge"
							class:ok={job.status === 'done'}
							class:bad={job.status === 'failed'}
							class:live={job.status === 'running'}
						>
							{job.error === 'cancelled' ? 'спряна' : statusText[job.status]}
						</span>
					</div>
					<div class="prog">
						<div class="progress">
							<div style:width="{(100 * job.units_done) / job.units_total}%"></div>
						</div>
						<span class="muted count">{job.units_done}/{job.units_total}</span>
					</div>
					<div class="result">
						{#if job.template === 'pt'}
							<a href="{resolve('/app/n/[id]/live', { id })}?job={job.id}">На живо</a>
							{#if job.result}
								<span class="muted">{ptResult(job.result).width}×{ptResult(job.result).height}</span
								>
							{/if}
						{:else if job.result}
							<span>π ≈ <strong>{piResult(job.result).pi.toFixed(6)}</strong></span>
						{/if}
						{#if job.error && job.error !== 'cancelled'}
							<span class="muted small">{job.error}</span>
						{/if}
					</div>
					{#if admin && (job.status === 'queued' || job.status === 'running')}
						<ConfirmButton
							label="Спри"
							confirm="Спри задачата?"
							onconfirm={() => stop(job.id)}
							testid="stop-job"
						/>
					{/if}
				</li>
			{/each}
		</ul>
	{/if}
</section>

<style>
	section + section {
		margin-top: var(--s-4);
	}
	h2 {
		font-size: var(--text-lg);
		margin: 0 0 var(--s-3);
	}
	.new form {
		display: grid;
		gap: var(--s-4);
	}
	.new p {
		margin: 0;
	}
	.kinds {
		justify-self: start;
	}
	details summary {
		cursor: pointer;
		font-weight: 600;
		font-size: var(--text-sm);
	}
	.fields {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
		gap: var(--s-3);
		margin-top: var(--s-3);
	}
	.new button[type='submit'] {
		justify-self: start;
	}
	.jobs {
		list-style: none;
		margin: 0;
		padding: 0;
	}
	.job {
		display: grid;
		grid-template-columns: minmax(150px, 1fr) minmax(140px, 1fr) minmax(0, 1.2fr) auto;
		gap: var(--s-4);
		align-items: center;
		padding: 14px 0;
		border-bottom: 1px solid var(--border);
		font-size: var(--text-sm);
	}
	.job:last-child {
		border-bottom: 0;
	}
	.main {
		display: flex;
		gap: var(--s-3);
		align-items: center;
		flex-wrap: wrap;
	}
	.kind {
		display: inline-flex;
		gap: 8px;
		align-items: center;
		font-weight: 600;
	}
	.prog {
		display: flex;
		gap: var(--s-3);
		align-items: center;
	}
	.prog .progress {
		flex: 1;
	}
	.count {
		font-size: var(--text-xs);
		font-variant-numeric: tabular-nums;
	}
	.result {
		display: grid;
		gap: 2px;
		min-width: 0;
		overflow-wrap: anywhere;
	}
	.small {
		font-size: var(--text-xs);
	}
	@media (max-width: 800px) {
		.job {
			grid-template-columns: minmax(0, 1fr) auto;
			gap: var(--s-2) var(--s-3);
		}
		.prog,
		.result {
			grid-column: 1 / -1;
		}
	}
</style>
