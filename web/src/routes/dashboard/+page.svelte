<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { LiveDashboard } from '#lib/dashboard/live.svelte.ts';
	import SwarmMap from '#lib/dashboard/SwarmMap.svelte';
	import { caughtDevices, deviceGroups, sortedJobs, unitsPerSecond } from '#lib/dashboard/state.ts';
	import type { JobStatus } from '#lib/protocol/JobStatus.ts';
	import type { PiResult } from '#lib/protocol/PiResult.ts';
	import type { PtResult } from '#lib/protocol/PtResult.ts';
	import type { ApiError } from '#lib/protocol/ApiError.ts';

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
	const PASSWORD_KEY = 'swarm-submit-password';
	let password = $state('');
	onMount(() => {
		try {
			password = localStorage.getItem(PASSWORD_KEY) ?? '';
		} catch {
			// Storage may be unavailable (private mode); the field still works.
		}
	});

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		submitting = true;
		submitError = null;
		try {
			const headers: Record<string, string> = { 'content-type': 'application/json' };
			if (password) headers.authorization = `Bearer ${password}`;
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
			try {
				localStorage.setItem(PASSWORD_KEY, password);
			} catch {
				// Not remembered; nothing else depends on it.
			}
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
			headers: password ? { authorization: `Bearer ${password}` } : {}
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

<main>
	<h1>Табло</h1>
	<p class="muted" data-testid="connection">
		{connected ? 'Свързано с координатора' : 'Няма връзка с координатора…'}
	</p>

	<div class="grid">
		<div class="card stat">
			<div class="value" data-testid="worker-count">{workers.length}</div>
			<div class="label">Устройства в мрежата</div>
		</div>
		<div class="card stat">
			<div class="value">{rate.toFixed(1)}</div>
			<div class="label">Единици в секунда</div>
		</div>
		<div class="card stat">
			<div class="value">{jobs.filter((j) => j.status === 'running').length}</div>
			<div class="label">Задачи в изпълнение</div>
		</div>
	</div>

	<section class="card">
		<h2>Задачи</h2>
		{#if jobs.length === 0}
			<p class="muted">Все още няма задачи.</p>
		{:else}
			<div class="table-wrap">
				<table>
					<thead>
						<tr><th>Шаблон</th><th>Състояние</th><th>Напредък</th><th>Резултат</th></tr>
					</thead>
					<tbody>
						{#each jobs as job (job.id)}
							<tr data-testid="job" data-job-id={job.id} data-status={job.status}>
								<td>{job.template}</td>
								<td class:ok={job.status === 'done'} class:bad={job.status === 'failed'}>
									{job.error === CANCELLED ? 'спряна' : statusText[job.status]}
									{#if job.error && job.error !== CANCELLED}
										<div class="muted job-error">{job.error}</div>
									{/if}
									{#if job.status === 'queued' || job.status === 'running'}
										<button
											class="secondary stop"
											onclick={() => stopJob(job.id)}
											data-testid="stop-job">Спри</button
										>
									{/if}
								</td>
								<td>
									<div class="progress-cell">
										<div class="progress">
											<div style:width="{(100 * job.units_done) / job.units_total}%"></div>
										</div>
										<span class="muted">{job.units_done}/{job.units_total}</span>
									</div>
								</td>
								<td>
									{#if job.template === 'pt'}
										<a href={resolve('/render/[id]', { id: job.id })}>Изображение</a>
										{#if job.result}
											<span class="muted">
												{ptResult(job.result).width}×{ptResult(job.result).height}, {ptResult(
													job.result
												).samples_per_pixel} проби/пиксел
											</span>
										{/if}
									{:else if job.result && job.template === 'pi'}
										π ≈ <strong>{piResult(job.result).pi.toFixed(6)}</strong>
										<span class="muted">
											(грешка {Math.abs(piResult(job.result).pi - Math.PI).toExponential(1)})
										</span>
									{:else}
										—
									{/if}
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{/if}
		{#if stopError}<p class="bad" role="alert">{stopError}</p>{/if}
	</section>

	<section class="card">
		<h2>Устройства</h2>
		{#if workers.length === 0}
			<p class="muted">
				Няма включени устройства. Отворете <a href={resolve('/worker')}>„Включи се“</a> на друго устройство.
			</p>
		{:else}
			<div class="map"><SwarmMap {workers} {caught} /></div>
			<div class="table-wrap">
				<table>
					<thead>
						<tr><th>Устройство</th><th>Среда</th><th>Брой</th><th>Единици</th></tr>
					</thead>
					<tbody>
						{#each groups as g (g.key)}
							<tr data-testid="device-group">
								<td>{g.vendor} · {g.architecture}</td>
								<td>{g.backend}</td>
								<td>{g.count}</td>
								<td>{g.units_done}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{/if}
	</section>

	<section class="card">
		<h2>Нова задача</h2>
		<form onsubmit={submit} class="form">
			<label>
				Шаблон
				<select bind:value={template} data-testid="template">
					<option value="pt">Трасиране на лъчи (изображение)</option>
					<option value="pi">π по метода Монте Карло</option>
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
			<label>
				Парола
				<input
					type="password"
					autocomplete="current-password"
					placeholder="ако е зададена"
					bind:value={password}
					data-testid="password"
				/>
			</label>
			<button type="submit" disabled={submitting}>Пусни</button>
		</form>
		{#if submitError}<p class="bad" role="alert">{submitError}</p>{/if}
	</section>
</main>

<style>
	section {
		margin-top: 16px;
	}
	.map {
		max-width: 560px;
		margin-bottom: 12px;
	}
	.stop {
		margin-left: 8px;
		padding: 2px 10px;
		font-size: 0.8125rem;
	}
	.job-error {
		font-size: 0.8125rem;
	}
	.progress-cell {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	.progress-cell .progress {
		flex: 1;
	}
	.form {
		display: grid;
		gap: 12px;
		grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
		align-items: end;
	}
</style>
