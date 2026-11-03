<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { page } from '$app/state';
	import { nnShader, nnVectors, piShader, piVectors, ptShader, ptVectors } from '#lib/templates.ts';
	import { sha256Hex } from '#lib/gpu/hash.ts';
	import { agreement, decodeRgb, gpuAgrees, type Agreement } from '#lib/pt.ts';
	import { ComputeClient } from '#lib/worker/compute-client.ts';
	import type { DeviceInfo } from '#lib/protocol/DeviceInfo.ts';
	import type { PiUnitOutput } from '#lib/protocol/PiUnitOutput.ts';
	import type { PiUnitParams } from '#lib/protocol/PiUnitParams.ts';
	import type { PtUnitOutput } from '#lib/protocol/PtUnitOutput.ts';
	import type { PtUnitParams } from '#lib/protocol/PtUnitParams.ts';
	import type { TemplateRef } from '#lib/protocol/TemplateRef.ts';
	import type { NnUnitOutput } from '#lib/protocol/NnUnitOutput.ts';
	import type { NnUnitParams } from '#lib/protocol/NnUnitParams.ts';
	import Icon from '#lib/ui/Icon.svelte';

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

	interface NnRow {
		params: NnUnitParams;
		expected: NnUnitOutput;
		actual: NnUnitOutput | null;
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
	// Network training is integer-only: exact on the GPU as on the CPU (spec 0011).
	let nnRows = $state<NnRow[]>(
		nnVectors.map(({ correct, curve, digest, ...params }) => ({
			params,
			expected: { correct, curve, digest },
			actual: null
		}))
	);
	const nnSame = (a: NnUnitOutput | null, b: NnUnitOutput) =>
		!!a &&
		a.correct === b.correct &&
		a.digest === b.digest &&
		a.curve.length === b.curve.length &&
		a.curve.every((v, i) => v === b.curve[i]);
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
			const nn: TemplateRef = { id: 'nn', version: 1, sha256: await sha256Hex(nnShader) };
			for (const row of nnRows) {
				row.actual = (await client.run(nn, $state.snapshot(row.params))).output as NnUnitOutput;
			}
			const piOk = piRows.length > 0 && piRows.every((r) => r.actual === r.expected);
			const ptOk = ptRows.length > 0 && ptRows.every(ptPasses);
			const nnOk = nnRows.length > 0 && nnRows.every((r) => nnSame(r.actual, r.expected));
			result = piOk && ptOk && nnOk ? 'pass' : 'fail';

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

<svelte:head>
	<title>Самопроверка · Рояк</title>
</svelte:head>

<main class="selftest">
	<header class="page-head">
		<span class="eyebrow">Самопроверка</span>
		<h1>Смята ли това устройство точно?</h1>
		<p>
			Устройството изчислява задачи с предварително известен отговор и ги сравнява с еталона:
			побитово на процесора, а на видеокартата — с малък допуск за закръгляне.
		</p>
	</header>

	<section
		class="card verdict"
		class:pass={result === 'pass'}
		class:fail={result === 'fail' || result === 'unsupported'}
	>
		<span class="verdict-icon" aria-hidden="true">
			{#if result === 'running'}
				<span class="spinner"></span>
			{:else if result === 'pass'}
				<Icon name="check" size={28} />
			{:else}
				<Icon name="alert" size={28} />
			{/if}
		</span>
		<div class="verdict-text">
			<p
				data-testid="selftest-status"
				data-result={result}
				data-backend={device?.backend}
				class:ok={result === 'pass'}
				class:bad={result === 'fail' || result === 'unsupported'}
			>
				<strong>{resultText[result]}</strong>
			</p>
			{#if error}<p class="bad small">{error}</p>{/if}
		</div>
	</section>

	<div class="facts">
		<div class="card stat">
			<div class="value small-value">
				{device ? (device.backend === 'webgpu' ? 'Видеокарта' : 'Процесор') : '—'}
			</div>
			<div class="label">{device ? `${device.vendor} · ${device.architecture}` : 'устройство'}</div>
		</div>
		<div class="card stat">
			<div class="value">
				{samplesPerSecond === null ? '—' : `${(samplesPerSecond / 1e6).toFixed(0)} млн.`}
			</div>
			<div class="label">проби за π в секунда</div>
		</div>
		<div class="card stat">
			<div class="value">
				{pathsPerSecond === null ? '—' : `${(pathsPerSecond / 1e6).toFixed(2)} млн.`}
			</div>
			<div class="label">пътя на светлината в секунда</div>
		</div>
	</div>

	<details class="card more">
		<summary>Подробности за проверката</summary>
		<p class="muted small ua">Браузър: <code>{navigator.userAgent}</code></p>

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

		<h2>Проследяване на лъчи</h2>
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

		<h2>Невронна мрежа</h2>
		<div class="table-wrap">
			<table>
				<thead>
					<tr><th>Мрежа</th><th>Обучение</th><th>Очаквано</th><th>Получено</th></tr>
				</thead>
				<tbody>
					{#each nnRows as r, i (i)}
						<tr data-testid="nn-row" data-same={nnSame(r.actual, r.expected)}>
							<td>{r.params.layers} × {r.params.hidden}</td>
							<td>{r.params.steps} стъпки · партида {2 ** r.params.batch_log2}</td>
							<td>{r.expected.correct} верни · {r.expected.digest.slice(0, 8)}</td>
							<td
								class:ok={nnSame(r.actual, r.expected)}
								class:bad={r.actual && !nnSame(r.actual, r.expected)}
							>
								{#if !r.actual}
									…
								{:else if nnSame(r.actual, r.expected)}
									побитово точно
								{:else}
									{r.actual.correct} верни · {r.actual.digest.slice(0, 8)}
								{/if}
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	</details>
</main>

<style>
	.selftest {
		max-width: 900px;
		display: grid;
		gap: var(--s-4);
	}
	.verdict {
		display: flex;
		align-items: center;
		gap: var(--s-4);
		padding: var(--s-5);
		transition: background-color var(--slow) var(--ease);
	}
	.verdict p {
		margin: 0;
		font-size: var(--text-lg);
	}
	.verdict-icon {
		flex: none;
		display: grid;
		place-items: center;
		width: 56px;
		height: 56px;
		border-radius: 50%;
		background: rgb(118 118 128 / 0.12);
		color: var(--muted);
		transition:
			background-color var(--slow) var(--ease),
			color var(--slow) var(--ease),
			transform var(--slow) var(--ease);
	}
	.pass .verdict-icon {
		background: var(--ok-soft);
		color: var(--ok);
		transform: scale(1.04);
	}
	.fail .verdict-icon {
		background: var(--bad-soft);
		color: var(--bad);
	}
	.spinner {
		width: 24px;
		height: 24px;
		border-radius: 50%;
		border: 3px solid rgb(118 118 128 / 0.25);
		border-top-color: var(--brand);
		animation: turn 0.9s linear infinite;
	}
	@keyframes turn {
		to {
			transform: rotate(360deg);
		}
	}
	.small {
		font-size: var(--text-sm);
	}
	.verdict .small {
		font-size: var(--text-sm);
		margin-top: 4px;
		overflow-wrap: anywhere;
	}
	.facts {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: var(--s-4);
	}
	.facts .label {
		overflow-wrap: anywhere;
	}
	.small-value {
		font-size: var(--text-lg);
	}
	.more summary {
		cursor: pointer;
		font-weight: 600;
		list-style-position: inside;
		min-height: 28px;
	}
	.more h2 {
		font-size: var(--text-md);
		margin: var(--s-5) 0 var(--s-2);
	}
	.ua {
		margin-top: var(--s-3);
	}
	@media (max-width: 640px) {
		.facts {
			grid-template-columns: 1fr;
		}
	}
</style>
