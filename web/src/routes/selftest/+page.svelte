<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { page } from '$app/state';
	import { piShader, piVectors as vectors } from '#lib/templates.ts';
	import { sha256Hex } from '#lib/gpu/hash.ts';
	import { ComputeClient } from '#lib/worker/compute-client.ts';
	import type { DeviceInfo } from '#lib/protocol/DeviceInfo.ts';
	import type { PiUnitOutput } from '#lib/protocol/PiUnitOutput.ts';
	import type { PiUnitParams } from '#lib/protocol/PiUnitParams.ts';
	import type { TemplateRef } from '#lib/protocol/TemplateRef.ts';

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

	// Runs through the same compute worker as real units: on the GPU when there is
	// one, otherwise on the CPU. `?backend=wasm` tests the CPU path on any device.
	const client = new ComputeClient((reason) => (error = `GPU device lost: ${reason}`));
	onDestroy(() => client.terminate());

	onMount(async () => {
		try {
			device = await client.init(
				page.url.searchParams.get('backend') === 'wasm' ? 'wasm' : undefined
			);
			const template: TemplateRef = { id: 'pi', version: 1, sha256: await sha256Hex(piShader) };
			// Copies the fields: `rows` are reactive proxies, which postMessage cannot clone.
			const run = async ({ seed, unit_index, samples }: PiUnitParams) =>
				(await client.run(template, { seed, unit_index, samples })).output as PiUnitOutput;

			for (const row of rows) {
				row.actual = (await run(row)).inside;
			}
			result = rows.length > 0 && rows.every((r) => r.actual === r.expected) ? 'pass' : 'fail';

			// Warm-up, then a timed run.
			await run({ seed: 1, unit_index: 0, samples: 1 << 20 });
			const { elapsed_ms } = await client.run(template, {
				seed: 1,
				unit_index: 1,
				samples: BENCH_SAMPLES
			});
			samplesPerSecond = BENCH_SAMPLES / (Math.max(elapsed_ms, 1) / 1000);
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
			result = device ? 'fail' : 'unsupported';
		}
	});

	const processor = $derived(device?.backend === 'webgpu' ? 'видеокартата' : 'процесорът');
	const resultText = $derived({
		running: 'Проверява се…',
		pass: `Всички проверки са успешни: ${processor} смята точно.`,
		fail: 'Има разминаване с еталонните резултати!',
		unsupported: 'Този браузър не може да изчислява за Рояк.'
	});
</script>

<main>
	<h1>Самопроверка</h1>
	<p class="muted">
		Изпълнява изчислителното ядро върху еталонни входове и сравнява резултата побитово с
		референтната реализация.
	</p>

	<div class="card">
		<p
			data-testid="selftest-status"
			data-result={result}
			data-backend={device?.backend}
			class:ok={result === 'pass'}
			class:bad={result === 'fail' || result === 'unsupported'}
		>
			<strong>{resultText[result]}</strong>
		</p>
		{#if error}<p class="bad">{error}</p>{/if}
		<p class="muted">
			Устройство: {device
				? `${device.backend === 'webgpu' ? 'видеокарта' : 'процесор'}: ${device.vendor} · ${device.architecture}`
				: '—'}<br />
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
