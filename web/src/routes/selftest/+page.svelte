<script lang="ts">
	import { onMount } from 'svelte';
	import { piVectors as vectors } from '#lib/templates.ts';
	import { requestGpu } from '#lib/gpu/device.ts';
	import { createPiKernel } from '#lib/gpu/pi.ts';
	import type { DeviceInfo } from '#lib/protocol/DeviceInfo.ts';

	interface Row {
		seed: number;
		unit_index: number;
		samples: number;
		expected: number;
		actual: number | null;
	}

	const BENCH_SAMPLES = 1 << 26;

	let device = $state<DeviceInfo | null>(null);
	let rows = $state<Row[]>(vectors.map((v) => ({ ...v, expected: v.inside, actual: null })));
	let result = $state<'running' | 'pass' | 'fail' | 'unsupported'>('running');
	let error = $state<string | null>(null);
	let samplesPerSecond = $state<number | null>(null);

	onMount(async () => {
		try {
			const gpu = await requestGpu();
			device = gpu.info;
			const kernel = createPiKernel(gpu.device);
			for (const row of rows) {
				row.actual = (await kernel.run(row)).inside;
			}
			result = rows.length > 0 && rows.every((r) => r.actual === r.expected) ? 'pass' : 'fail';

			// Warm-up, then a timed run.
			await kernel.run({ seed: 1, unit_index: 0, samples: 1 << 20 });
			const started = performance.now();
			await kernel.run({ seed: 1, unit_index: 1, samples: BENCH_SAMPLES });
			samplesPerSecond = BENCH_SAMPLES / ((performance.now() - started) / 1000);
			kernel.destroy();
			gpu.device.destroy();
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
			result = device ? 'fail' : 'unsupported';
		}
	});

	const resultText = {
		running: 'Проверява се…',
		pass: 'Всички проверки са успешни: видеокартата смята точно.',
		fail: 'Има разминаване с еталонните резултати!',
		unsupported: 'WebGPU не е достъпен на това устройство.'
	};
</script>

<main>
	<h1>Самопроверка на видеокартата</h1>
	<p class="muted">
		Изпълнява изчислителното ядро върху еталонни входове и сравнява резултата побитово с
		референтната реализация.
	</p>

	<div class="card">
		<p
			data-testid="selftest-status"
			data-result={result}
			class:ok={result === 'pass'}
			class:bad={result === 'fail' || result === 'unsupported'}
		>
			<strong>{resultText[result]}</strong>
		</p>
		{#if error}<p class="bad">{error}</p>{/if}
		<p class="muted">
			Устройство: {device ? `${device.vendor} · ${device.architecture}` : '—'}<br />
			Скорост:
			{samplesPerSecond === null ? '—' : `${(samplesPerSecond / 1e6).toFixed(0)} млн. проби/с`}<br
			/>
			Браузър: <code>{navigator.userAgent}</code>
		</p>
	</div>

	<section class="card">
		<div class="table-wrap">
			<table>
				<thead>
					<tr
						><th>Начално число</th><th>Единица</th><th>Проби</th><th>Очаквано</th><th>Получено</th
						></tr
					>
				</thead>
				<tbody>
					{#each rows as r, i (i)}
						<tr>
							<td>{r.seed}</td>
							<td>{r.unit_index}</td>
							<td>{r.samples}</td>
							<td>{r.expected}</td>
							<td
								class:ok={r.actual === r.expected}
								class:bad={r.actual !== null && r.actual !== r.expected}
							>
								{r.actual ?? '…'}
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	</section>
</main>

<style>
	section {
		margin-top: 16px;
	}
</style>
