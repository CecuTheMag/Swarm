<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { LiveDashboard } from '#lib/dashboard/live.svelte.ts';
	import SwarmMap from '#lib/dashboard/SwarmMap.svelte';
	import {
		caughtDevices,
		deviceGroups,
		deviceLabel,
		sortedJobs,
		unitsPerSecond
	} from '#lib/dashboard/state.ts';
	import type { Backend } from '#lib/protocol/Backend.ts';
	import type { JobStatus } from '#lib/protocol/JobStatus.ts';
	import type { PiResult } from '#lib/protocol/PiResult.ts';
	import type { PtResult } from '#lib/protocol/PtResult.ts';
	import type { ApiError } from '#lib/protocol/ApiError.ts';
	import { authHeaders, loadPassword, savePassword } from '#lib/password.ts';
	import Icon from '#lib/ui/Icon.svelte';

	const live = new LiveDashboard();
	onMount(() => live.start());
	onDestroy(() => live.stop());
	const dash = $derived(live.state);
	const connected = $derived(live.connected);
	const now = $derived(live.now);

	const workers = $derived(Object.values(dash.workers));
	const groups = $derived(deviceGroups(dash));
	const caught = $derived(caughtDevices(dash, now));
	const jobs = $derived(sortedJobs(dash));
	const rate = $derived(unitsPerSecond(dash, now));

	const statusText: Record<JobStatus, string> = {
		queued: 'в опашка',
		running: 'изпълнява се',
		done: 'готова',
		failed: 'неуспешна'
	};

	let template = $state<'pt' | 'pi'>('pt');
	let units = $state(64);
	let samples = $state(16_777_216);
	let seed = $state(42);
	// 16:9 at 1024 samples per pixel: 240 tiles of 64 pixels, 8 passes of 128 samples.
	// A unit is about 0.5 M light paths (a phone: ~0.1 s, a CPU thread: ~0.4 s) for a
	// 65 KiB result, so neither round trips nor bandwidth dominate.
	let width = $state(1280);
	let height = $state(720);
	let tile = $state(64);
	let spp = $state(128);
	let passes = $state(8);
	let submitError = $state<string | null>(null);
	let submitting = $state(false);

	// Only the organiser starts jobs on a public address (SWARM_SUBMIT_TOKEN); the
	// password is kept on this device so it is typed once.
	let password = $state('');
	onMount(() => {
		password = loadPassword();
	});

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		submitting = true;
		submitError = null;
		try {
			const headers = { 'content-type': 'application/json', ...authHeaders(password) };
			const res = await fetch('/api/jobs', {
				method: 'POST',
				headers,
				body: JSON.stringify(
					template === 'pi'
						? { template, params: { units, samples_per_unit: samples, seed } }
						: { template, params: { width, height, tile, spp, passes, seed } }
				)
			});
			if (res.status === 401) {
				submitError = password ? 'Грешна парола.' : 'Нужна е парола, за да пуснете задача.';
				return;
			}
			savePassword(password);
			if (!res.ok) submitError = ((await res.json()) as ApiError).error;
			else if (template === 'pt') {
				await goto(resolve('/render/[id]', { id: ((await res.json()) as { id: string }).id }));
			}
		} catch (e) {
			submitError = e instanceof Error ? e.message : String(e);
		} finally {
			submitting = false;
		}
	}

	// Stopping a job needs the same password as starting one.
	let stopError = $state<string | null>(null);
	async function stopJob(id: string) {
		stopError = null;
		const res = await fetch(`/api/jobs/${id}/cancel`, {
			method: 'POST',
			headers: authHeaders(password)
		});
		if (res.status === 401) {
			stopError = password ? 'Грешна парола.' : 'Нужна е парола, за да спрете задача.';
		} else if (!res.ok) {
			stopError = ((await res.json()) as ApiError).error;
		}
	}
	// The coordinator's error for a stopped job (db::CANCELLED).
	const CANCELLED = 'cancelled';

	const piResult = (result: unknown) => result as PiResult;
	const ptResult = (result: unknown) => result as PtResult;
</script>

<svelte:head>
	<title>Табло · Рояк</title>
</svelte:head>

<main class="dash">
	<header class="page-head head">
		<div>
			<span class="eyebrow">Табло</span>
			<h1>Мрежата на живо</h1>
		</div>
		<span class="badge" class:live={connected} class:bad={!connected}>
			<span class="dot" class:pulse={connected}></span>
			<span data-testid="connection">
				{connected ? 'Свързано с координатора' : 'Няма връзка с координатора…'}
			</span>
		</span>
	</header>

	<div class="stats">
		<div class="card stat">
			<div class="value" data-testid="worker-count">{workers.length}</div>
			<div class="label">устройства в мрежата</div>
		</div>
		<div class="card stat">
			<div class="value">{rate.toFixed(1)}</div>
			<div class="label">изчислени части в секунда</div>
		</div>
		<div class="card stat">
			<div class="value">{jobs.filter((j) => j.status === 'running').length}</div>
			<div class="label">задачи в изпълнение</div>
		</div>
	</div>

	<div class="cols">
		<section class="card new" aria-labelledby="new-title">
			<h2 id="new-title">Нова задача</h2>
			<form onsubmit={submit} class="form">
				<label class="wide">
					Шаблон
					<select bind:value={template} data-testid="template">
						<option value="pt">Изображение (проследяване на лъчи)</option>
						<option value="pi">Числото π (метод Монте Карло)</option>
					</select>
				</label>
				{#if template === 'pi'}
					<label>Единици <input type="number" min="1" max="10000" bind:value={units} /></label>
					<label>
						Проби на единица
						<input type="number" min="1" max="1073741824" bind:value={samples} />
					</label>
				{:else}
					<label>Ширина <input type="number" min="16" max="2048" bind:value={width} /></label>
					<label>Височина <input type="number" min="16" max="2048" bind:value={height} /></label>
					<label>Плочка <input type="number" min="8" max="128" bind:value={tile} /></label>
					<label>Проби на проход <input type="number" min="1" max="1024" bind:value={spp} /></label>
					<label>Проходи <input type="number" min="1" max="256" bind:value={passes} /></label>
				{/if}
				<label>Начално число <input type="number" min="0" bind:value={seed} /></label>
				<label class="wide">
					Парола
					<input
						type="password"
						autocomplete="current-password"
						placeholder="ако е зададена"
						bind:value={password}
						data-testid="password"
					/>
				</label>
				<button type="submit" class="wide lg" disabled={submitting}>
					{#if template === 'pt'}<Icon name="image" size={18} />{:else}<Icon
							name="atom"
							size={18}
						/>{/if}
					Пусни
				</button>
			</form>
			{#if submitError}
				<p class="notice bad" role="alert">{submitError}</p>
			{/if}
		</section>

		<section class="card devices" aria-labelledby="devices-title">
			<h2 id="devices-title">Устройства</h2>
			{#if workers.length === 0}
				<div class="empty">
					<Icon name="phone" size={28} />
					<p class="muted">
						Няма включени устройства. Отворете <a href={resolve('/worker')}>„Включи се“</a> на друго устройство.
					</p>
				</div>
			{:else}
				<div class="map"><SwarmMap {workers} {caught} /></div>
				<ul class="groups">
					{#each groups as g (g.key)}
						<li data-testid="device-group">
							<span class="group-name">
								<strong>{g.count} ×</strong>
								{deviceLabel({
									backend: g.backend as Backend,
									vendor: g.vendor,
									architecture: g.architecture
								})}
								<span class="muted">· {g.vendor} · {g.architecture}</span>
							</span>
							<span class="muted">{g.units_done} части</span>
						</li>
					{/each}
				</ul>
			{/if}
		</section>
	</div>

	<section class="card jobs-card" aria-labelledby="jobs-title">
		<h2 id="jobs-title">Задачи</h2>
		{#if jobs.length === 0}
			<p class="muted">Все още няма задачи. Пуснете първата отгоре.</p>
		{:else}
			<ul class="jobs">
				{#each jobs as job (job.id)}
					<li class="job" data-testid="job" data-job-id={job.id} data-status={job.status}>
						<div class="job-main">
							<span class="job-kind">
								<Icon name={job.template === 'pt' ? 'image' : 'atom'} size={18} />
								{job.template === 'pt' ? 'Изображение' : 'Числото π'}
							</span>
							<span
								class="badge"
								class:ok={job.status === 'done'}
								class:bad={job.status === 'failed'}
								class:live={job.status === 'running'}
							>
								{job.error === CANCELLED ? 'спряна' : statusText[job.status]}
							</span>
						</div>
						<div class="job-progress">
							<div class="progress">
								<div style:width="{(100 * job.units_done) / job.units_total}%"></div>
							</div>
							<span class="muted count">{job.units_done}/{job.units_total}</span>
						</div>
						<div class="job-result">
							{#if job.template === 'pt'}
								<a href={resolve('/render/[id]', { id: job.id })}>Виж изображението</a>
								{#if job.result}
									<span class="muted">
										{ptResult(job.result).width}×{ptResult(job.result).height}, {ptResult(
											job.result
										).samples_per_pixel} проби/пиксел
									</span>
								{/if}
							{:else if job.result && job.template === 'pi'}
								<span>π ≈ <strong>{piResult(job.result).pi.toFixed(6)}</strong></span>
								<span class="muted">
									грешка {Math.abs(piResult(job.result).pi - Math.PI).toExponential(1)}
								</span>
							{:else}
								<span class="muted">—</span>
							{/if}
							{#if job.error && job.error !== CANCELLED}
								<span class="muted job-error">{job.error}</span>
							{/if}
						</div>
						{#if job.status === 'queued' || job.status === 'running'}
							<button class="secondary sm" onclick={() => stopJob(job.id)} data-testid="stop-job">
								Спри
							</button>
						{/if}
					</li>
				{/each}
			</ul>
		{/if}
		{#if stopError}<p class="notice bad" role="alert">{stopError}</p>{/if}
	</section>
</main>

<style>
	.dash {
		display: grid;
		gap: var(--s-4);
	}
	.head {
		display: flex;
		align-items: flex-end;
		justify-content: space-between;
		gap: var(--s-4);
		flex-wrap: wrap;
		margin-bottom: var(--s-3);
	}
	.head h1 {
		margin: 0;
	}
	.stats {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: var(--s-4);
	}
	.cols {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
		gap: var(--s-4);
		align-items: start;
	}
	h2 {
		font-size: var(--text-lg);
		margin-bottom: var(--s-4);
	}
	.form {
		display: grid;
		gap: var(--s-3);
		grid-template-columns: repeat(2, minmax(0, 1fr));
		align-items: end;
	}
	.form .wide {
		grid-column: 1 / -1;
	}
	.new .notice {
		margin: var(--s-3) 0 0;
	}

	.map {
		max-width: 520px;
		margin-bottom: var(--s-4);
	}
	.empty {
		display: grid;
		justify-items: center;
		gap: var(--s-2);
		padding: var(--s-6) var(--s-4);
		text-align: center;
		color: var(--faint);
		border: 1px dashed var(--border-strong);
		border-radius: var(--radius);
	}
	.empty p {
		margin: 0;
	}
	.groups {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: 2px;
		font-size: var(--text-sm);
	}
	.groups li {
		display: flex;
		justify-content: space-between;
		gap: var(--s-3);
		padding: 10px 0;
		border-bottom: 1px solid var(--border);
	}
	.groups li:last-child {
		border-bottom: 0;
	}
	.group-name {
		min-width: 0;
		overflow-wrap: anywhere;
	}

	.jobs {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
	}
	.job {
		display: grid;
		grid-template-columns: minmax(150px, 1.1fr) minmax(140px, 1fr) minmax(0, 1.4fr) auto;
		align-items: center;
		gap: var(--s-4);
		padding: 14px 0;
		border-bottom: 1px solid var(--border);
		font-size: var(--text-sm);
		animation: row-in var(--slow) var(--ease) both;
	}
	.job:last-child {
		border-bottom: 0;
	}
	@keyframes row-in {
		from {
			opacity: 0;
			transform: translateY(6px);
		}
	}
	.job-main {
		display: flex;
		align-items: center;
		gap: var(--s-3);
		flex-wrap: wrap;
	}
	.job-kind {
		display: inline-flex;
		align-items: center;
		gap: 8px;
		font-weight: 600;
	}
	.job-kind :global(svg) {
		color: var(--brand-ink);
	}
	.job-progress {
		display: flex;
		align-items: center;
		gap: var(--s-3);
	}
	.job-progress .progress {
		flex: 1;
	}
	.count {
		font-variant-numeric: tabular-nums;
		font-size: var(--text-xs);
	}
	.job-result {
		display: grid;
		gap: 2px;
		min-width: 0;
		overflow-wrap: anywhere;
	}
	.job-error {
		font-size: var(--text-xs);
	}
	.jobs-card .notice {
		margin: var(--s-3) 0 0;
	}

	@media (max-width: 900px) {
		.cols {
			grid-template-columns: 1fr;
		}
		.job {
			grid-template-columns: minmax(0, 1fr) auto;
			gap: var(--s-2) var(--s-3);
		}
		.job-progress,
		.job-result {
			grid-column: 1 / -1;
		}
		.job > button {
			grid-row: 1;
			grid-column: 2;
		}
	}
	@media (max-width: 600px) {
		.stats {
			grid-template-columns: 1fr 1fr;
		}
		.stats .card:last-child {
			grid-column: 1 / -1;
		}
		.form {
			grid-template-columns: 1fr 1fr;
		}
	}
</style>
