<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import { apply, emptyState, sortedJobs, unitsPerSecond } from '#lib/dashboard/state.ts';
	import { wsUrl } from '#lib/net.ts';
	import type { DashboardEvent } from '#lib/protocol/DashboardEvent.ts';
	import type { JobStatus } from '#lib/protocol/JobStatus.ts';
	import type { PiResult } from '#lib/protocol/PiResult.ts';
	import type { ApiError } from '#lib/protocol/ApiError.ts';

	let dash = $state(emptyState());
	let now = $state(Date.now());
	let connected = $state(false);

	let ws: WebSocket | null = null;
	let closed = false;
	let retry: ReturnType<typeof setTimeout> | undefined;
	let tick: ReturnType<typeof setInterval> | undefined;

	function connect() {
		const socket = new WebSocket(wsUrl('/ws/dashboard'));
		ws = socket;
		socket.onopen = () => (connected = true);
		socket.onmessage = (e) => {
			dash = apply(dash, JSON.parse(e.data) as DashboardEvent, Date.now());
		};
		socket.onclose = () => {
			connected = false;
			if (!closed) retry = setTimeout(connect, 2000);
		};
	}

	onMount(() => {
		connect();
		tick = setInterval(() => (now = Date.now()), 1000);
	});
	onDestroy(() => {
		closed = true;
		clearTimeout(retry);
		clearInterval(tick);
		ws?.close();
	});

	const workers = $derived(Object.values(dash.workers));
	const jobs = $derived(sortedJobs(dash));
	const rate = $derived(unitsPerSecond(dash, now));

	const statusText: Record<JobStatus, string> = {
		queued: 'в опашка',
		running: 'изпълнява се',
		done: 'готова',
		failed: 'неуспешна'
	};

	let units = $state(64);
	let samples = $state(16_777_216);
	let seed = $state(42);
	let submitError = $state<string | null>(null);
	let submitting = $state(false);

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		submitting = true;
		submitError = null;
		try {
			const res = await fetch('/api/jobs', {
				method: 'POST',
				headers: { 'content-type': 'application/json' },
				body: JSON.stringify({
					template: 'pi',
					params: { units, samples_per_unit: samples, seed }
				})
			});
			if (!res.ok) submitError = ((await res.json()) as ApiError).error;
		} catch (e) {
			submitError = e instanceof Error ? e.message : String(e);
		} finally {
			submitting = false;
		}
	}

	const piResult = (result: unknown) => result as PiResult;
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
									{statusText[job.status]}
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
									{#if job.result && job.template === 'pi'}
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
	</section>

	<section class="card">
		<h2>Устройства</h2>
		{#if workers.length === 0}
			<p class="muted">
				Няма включени устройства. Отворете <a href={resolve('/worker')}>„Включи се“</a> на друго устройство.
			</p>
		{:else}
			<div class="table-wrap">
				<table>
					<thead><tr><th>Устройство</th><th>Среда</th><th>Единици</th></tr></thead>
					<tbody>
						{#each workers as w (w.id)}
							<tr>
								<td>{w.device.vendor} · {w.device.architecture}</td>
								<td>{w.device.backend}</td>
								<td>{w.units_done}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{/if}
	</section>

	<section class="card">
		<h2>Нова задача: π по метода Монте Карло</h2>
		<form onsubmit={submit} class="form">
			<label>Единици <input type="number" min="1" max="10000" bind:value={units} /></label>
			<label>
				Проби на единица
				<input type="number" min="1" max="1073741824" bind:value={samples} />
			</label>
			<label>Начално число <input type="number" min="0" bind:value={seed} /></label>
			<button type="submit" disabled={submitting}>Пусни</button>
		</form>
		{#if submitError}<p class="bad" role="alert">{submitError}</p>{/if}
	</section>
</main>

<style>
	section {
		margin-top: 16px;
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
