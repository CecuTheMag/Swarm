<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { page } from '$app/state';
	import { piShader, piVectors, ptShader, ptVectors } from '#lib/templates.ts';
	import { sha256Hex } from '#lib/gpu/hash.ts';
	import { agreement, decodeRgb, gpuAgrees, type Agreement } from '#lib/pt.ts';
	import { ComputeClient } from '#lib/worker/compute-client.ts';
	import type { DeviceInfo } from '#lib/protocol/DeviceInfo.ts';
	import type { PiUnitOutput } from '#lib/protocol/PiUnitOutput.ts';
	import type { PiUnitParams } from '#lib/protocol/PiUnitParams.ts';
	import type { PtUnitOutput } from '#lib/protocol/PtUnitOutput.ts';
	import type { PtUnitParams } from '#lib/protocol/PtUnitParams.ts';
	import type { TemplateRef } from '#lib/protocol/TemplateRef.ts';

	interface PiRow {
		seed: number;
		unit_index: number;
		samples: number;
		expected: number;
		actual: number | null;
	}

	interface PtRow {
		params: PtUnitParams;
		expected: number[];
		/** Exact on the CPU; on the GPU, how closely it follows the reference. */
		exact: boolean | null;
		agreement: Agreement | null;
	}

	const PI_BENCH_SAMPLES = 1 << 26;
	const PT_BENCH: PtUnitParams = {
		width: 512,
		height: 288,
		x: 192,
		y: 112,
		w: 64,
		h: 64,
		spp: 16,
		pass: 0,
		seed: 1
	};

	let device = $state<DeviceInfo | null>(null);
	let piRows = $state<PiRow[]>(piVectors.map((v) => ({ ...v, expected: v.inside, actual: null })));
	let ptRows = $state<PtRow[]>(
		ptVectors.map(({ rgb, ...params }) => ({ params, expected: rgb, exact: null, agreement: null }))
	);
	let result = $state<'running' | 'pass' | 'fail' | 'unsupported'>('running');
	let error = $state<string | null>(null);
	let samplesPerSecond = $state<number | null>(null);
	let pathsPerSecond = $state<number | null>(null);

	// Runs through the same compute worker as real units: on the GPU when there is
	// one, otherwise on the CPU. `?backend=wasm` tests the CPU path on any device.
	const client = new ComputeClient((reason) => (error = `GPU device lost: ${reason}`));
	onDestroy(() => client.terminate());

	const ptPasses = (r: PtRow) =>
		device?.backend === 'webgpu' ? !!r.agreement && gpuAgrees(r.agreement) : r.exact === true;

	onMount(async () => {
		try {
			device = await client.init(
				page.url.searchParams.get('backend') === 'wasm' ? 'wasm' : undefined
			);
			const pi: TemplateRef = { id: 'pi', version: 1, sha256: await sha256Hex(piShader) };
			const pt: TemplateRef = { id: 'pt', version: 1, sha256: await sha256Hex(ptShader) };
			// Copies the fields: rows are reactive proxies, which postMessage cannot clone.
			const runPi = async ({ seed, unit_index, samples }: PiUnitParams) =>
				(await client.run(pi, { seed, unit_index, samples })).output as PiUnitOutput;
			const runPt = async (p: PtUnitParams) =>
				decodeRgb(((await client.run(pt, { ...p })).output as PtUnitOutput).rgb);

			for (const row of piRows) {
				row.actual = (await runPi(row)).inside;
			}
			for (const row of ptRows) {
				const sums = await runPt($state.snapshot(row.params));
				row.exact =
					sums.length === row.expected.length && sums.every((v, i) => v === row.expected[i]);
				row.agreement = agreement(sums, row.expected);
			}
			const piOk = piRows.length > 0 && piRows.every((r) => r.actual === r.expected);
			const ptOk = ptRows.length > 0 && ptRows.every(ptPasses);
			result = piOk && ptOk ? 'pass' : 'fail';

			// Warm-up, then timed runs.
			await runPi({ seed: 1, unit_index: 0, samples: 1 << 20 });
			const piRun = await client.run(pi, { seed: 1, unit_index: 1, samples: PI_BENCH_SAMPLES });
			samplesPerSecond = PI_BENCH_SAMPLES / (Math.max(piRun.elapsed_ms, 1) / 1000);
			await runPt({ ...PT_BENCH, spp: 1 });
			const ptRun = await client.run(pt, { ...PT_BENCH });
			pathsPerSecond =
				(PT_BENCH.w * PT_BENCH.h * PT_BENCH.spp) / (Math.max(ptRun.elapsed_ms, 1) / 1000);
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
	const mean = (values: number[], spp: number) =>
		values.reduce((a, b) => a + b, 0) / values.length / spp;
</script>

<main>
	<h1>Самопроверка</h1>
	<p class="muted">
		Изпълнява изчислителните ядра върху еталонни входове и сравнява резултата с референтната
		реализация: побитово на процесора, а на видеокартата — с допуск за закръгляне.
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
			π: {samplesPerSecond === null
				? '—'
				: `${(samplesPerSecond / 1e6).toFixed(0)} млн. проби/с`}<br />
			Трасиране на лъчи:
			{pathsPerSecond === null ? '—' : `${(pathsPerSecond / 1e6).toFixed(2)} млн. пътя/с`}<br />
			Браузър: <code>{navigator.userAgent}</code>
		</p>
	</div>

	<section class="card">
		<h2>π (Монте Карло)</h2>
		<div class="table-wrap">
			<table>
				<thead>
					<tr
						><th>Начално число</th><th>Единица</th><th>Проби</th><th>Очаквано</th><th>Получено</th
						></tr
					>
				</thead>
				<tbody>
					{#each piRows as r, i (i)}
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

	<section class="card">
		<h2>Трасиране на лъчи</h2>
		<div class="table-wrap">
			<table>
				<thead>
					<tr><th>Плочка</th><th>Проби</th><th>Средно очаквано</th><th>Получено</th></tr>
				</thead>
				<tbody>
					{#each ptRows as r, i (i)}
						<tr data-testid="pt-row">
							<td>
								{r.params.width}×{r.params.height} @ {r.params.x},{r.params.y} ({r.params.w}×{r
									.params.h})
							</td>
							<td>{r.params.spp} × проход {r.params.pass}</td>
							<td>{mean(r.expected, r.params.spp).toFixed(4)}</td>
							<td class:ok={r.agreement && ptPasses(r)} class:bad={r.agreement && !ptPasses(r)}>
								{#if !r.agreement}
									…
								{:else if r.exact}
									побитово точно
								{:else}
									{r.agreement.close}/{r.agreement.total} съвпадат, сума ±{(
										r.agreement.sumError * 100
									).toFixed(2)}%
								{/if}
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
