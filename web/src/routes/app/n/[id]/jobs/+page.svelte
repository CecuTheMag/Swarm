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
	import type { NnResult } from '#lib/protocol/NnResult.ts';

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
	let template = $state<'pt' | 'nn' | 'pi'>('pt');
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

	// A neural network sweep (spec 0011): which settings train best. Each preset is
	// a grid of settings; every device trains one combination.
	const SWEEPS = {
		quick: {
			hidden: [32, 64, 128],
			layers: [1],
			lr: [0.05, 0.2],
			batch: [32],
			steps: 300,
			seeds: 1
		},
		normal: {
			hidden: [32, 64, 128],
			layers: [1, 2],
			lr: [0.05, 0.1, 0.2, 0.4],
			batch: [32],
			steps: 600,
			seeds: 2
		},
		deep: {
			hidden: [16, 32, 64, 128, 256],
			layers: [1, 2],
			lr: [0.02, 0.05, 0.1, 0.2, 0.4],
			batch: [16, 32],
			steps: 600,
			seeds: 2
		}
	};
	let sweep = $state<keyof typeof SWEEPS>('normal');
	let nnHidden = $state('');
	let nnLayers = $state('');
	let nnLr = $state('');
	let nnBatch = $state('');
	let nnSteps = $state(600);
	let nnSeeds = $state(2);
	$effect(() => {
		const s = SWEEPS[sweep];
		nnHidden = s.hidden.join(', ');
		nnLayers = s.layers.join(', ');
		nnLr = s.lr.join(', ');
		nnBatch = s.batch.join(', ');
		nnSteps = s.steps;
		nnSeeds = s.seeds;
	});
	/** "32, 64 128" → [32, 64, 128]; values are separated by commas or spaces. */
	const list = (text: string) =>
		text
			.split(/[\s,;]+/)
			.filter((v) => v !== '')
			.map(Number)
			.filter((v) => Number.isFinite(v));
	const configs = $derived(
		list(nnHidden).length * list(nnLayers).length * list(nnLr).length * list(nnBatch).length
	);

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
						: template === 'nn'
							? {
									template,
									params: {
										dataset: 'glyphs',
										steps: nnSteps,
										hidden: list(nnHidden),
										layers: list(nnLayers),
										learning_rate: list(nnLr),
										batch: list(nnBatch),
										seeds: nnSeeds,
										seed
									}
								}
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
	const nnResult = (r: unknown) => r as NnResult;
	const kindName: Record<string, string> = {
		pt: 'Изображение',
		nn: 'Невронна мрежа',
		pi: 'Числото π'
	};
	const kindIcon = { pt: 'image', nn: 'network', pi: 'atom' } as const;
</script>

<svelte:head>
	<title>Задачи · {network.view?.name} · Рояк</title>
</svelte:head>

{#if admin}
	<section class="card new">
		<h2>Нова задача</h2>
		<form onsubmit={start}>
			<div class="segmented kinds stack-sm" role="group" aria-label="Какво да се изчисли">
				<button type="button" aria-pressed={template === 'pt'} onclick={() => (template = 'pt')}>
					<Icon name="image" size={16} /> Изображение
				</button>
				<button
					type="button"
					aria-pressed={template === 'nn'}
					onclick={() => (template = 'nn')}
					data-testid="kind-nn"
				>
					<Icon name="network" size={16} /> Невронна мрежа
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
			{:else if template === 'nn'}
				<p class="muted">
					Търси с кои настройки малка невронна мрежа се учи най-добре да разпознава ръкописни цифри.
					Всяко устройство обучава една настройка, а резултатите се подреждат в класация.
				</p>
				<div class="segmented stack-sm" role="group" aria-label="Обхват на търсенето">
					<button type="button" aria-pressed={sweep === 'quick'} onclick={() => (sweep = 'quick')}
						>Бързо сравнение</button
					>
					<button type="button" aria-pressed={sweep === 'normal'} onclick={() => (sweep = 'normal')}
						>Нормално</button
					>
					<button type="button" aria-pressed={sweep === 'deep'} onclick={() => (sweep = 'deep')}
						>Задълбочено</button
					>
				</div>
				<p class="muted small" data-testid="sweep-size">
					{configs} настройки × {nnSeeds}
					{nnSeeds === 1 ? 'обучение' : 'обучения'} = {configs * nnSeeds} части
				</p>
			{:else}
				<p class="muted">Числото π по метода Монте Карло: точен, проверим резултат.</p>
			{/if}

			<details bind:open={advanced}>
				<summary>Подробни настройки</summary>
				<div class="fields">
					{#if template === 'nn'}
						<label>Неврони в слой <input bind:value={nnHidden} inputmode="numeric" /></label>
						<label>Скрити слоеве <input bind:value={nnLayers} inputmode="numeric" /></label>
						<label>Скорост на учене <input bind:value={nnLr} inputmode="decimal" /></label>
						<label>Партида <input bind:value={nnBatch} inputmode="numeric" /></label>
						<label>Стъпки <input type="number" min="1" max="5000" bind:value={nnSteps} /></label>
						<label
							>Обучения на настройка <input
								type="number"
								min="1"
								max="5"
								bind:value={nnSeeds}
							/></label
						>
					{:else if template === 'pi'}
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
							<Icon name={kindIcon[job.template as keyof typeof kindIcon] ?? 'atom'} size={18} />
							{kindName[job.template] ?? job.template}
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
						{:else if job.template === 'nn'}
							<a
								href={resolve('/app/n/[id]/jobs/[job]', { id, job: job.id })}
								data-testid="sweep-link">Класация</a
							>
							{#if job.result && nnResult(job.result).rows.length > 0}
								<span class="muted"
									>най-добра: {(nnResult(job.result).rows[0].accuracy * 100).toFixed(1)} %</span
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
	/* Phones: three long choices read better as a column than squeezed in a row. */
	@media (max-width: 480px) {
		.stack-sm {
			grid-auto-flow: row;
		}
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
