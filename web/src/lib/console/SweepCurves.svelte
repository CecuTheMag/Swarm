<script lang="ts">
	import type { NnRow } from '#lib/protocol/NnRow.ts';

	// Training loss of the best configurations (spec 0011): lower is better. Each
	// configuration keeps its colour while it stays on the chart, whatever its rank.
	let { rows }: { rows: NnRow[] } = $props();

	const SHOWN = 5;
	// Drawn at the width it is shown at, so text and lines keep their pixel sizes.
	let width = $state(560);
	const W = $derived(Math.max(260, Math.round(width)));
	const H = 220;
	const PAD = { left: 40, right: 12, top: 12, bottom: 28 };

	// Colours are given to configurations, not ranks: a configuration that moves from
	// third to second keeps its line's colour.
	// Not reactive on purpose: it only remembers assignments between updates.
	let slots: Record<number, number> = {};
	const shown = $derived.by(() => {
		const top = rows.slice(0, SHOWN);
		const kept: Record<number, number> = {};
		for (const r of top) if (r.config in slots) kept[r.config] = slots[r.config];
		for (const r of top) {
			if (!(r.config in kept)) {
				const used = Object.values(kept);
				kept[r.config] = [0, 1, 2, 3, 4].find((s) => !used.includes(s)) ?? 0;
			}
		}
		slots = kept;
		return top.map((r, rank) => ({ row: r, rank, slot: kept[r.config] }));
	});

	const maxLoss = $derived(
		Math.max(0.5, ...shown.flatMap((s) => s.row.curve.filter((v) => v > 0)))
	);
	const x = (k: number, n: number) =>
		PAD.left + (k / Math.max(1, n - 1)) * (W - PAD.left - PAD.right);
	const y = (v: number) => PAD.top + (1 - v / maxLoss) * (H - PAD.top - PAD.bottom);
	const path = (curve: number[]) =>
		curve
			.map((v, k) => `${k === 0 ? 'M' : 'L'}${x(k, curve.length).toFixed(1)},${y(v).toFixed(1)}`)
			.join('');
	const ticks = $derived([0, maxLoss / 2, maxLoss]);

	const describe = (r: NnRow) =>
		`${r.layers} × ${r.hidden}, скорост ${r.learning_rate}, партида ${r.batch}`;

	// Hover: the nearest point of training, every line's value there.
	let hover = $state<number | null>(null);
	const points = $derived(shown[0]?.row.curve.length ?? 16);
	function move(event: PointerEvent) {
		const svg = event.currentTarget as SVGSVGElement;
		const box = svg.getBoundingClientRect();
		const px = ((event.clientX - box.left) / box.width) * W;
		const k = Math.round(((px - PAD.left) / (W - PAD.left - PAD.right)) * (points - 1));
		hover = Math.max(0, Math.min(points - 1, k));
	}
</script>

<div class="curves">
	<ul class="legend">
		{#each shown as s (s.row.config)}
			<li>
				<span class="swatch" style:--c="var(--s{s.slot + 1})"></span>
				<span class="rank">#{s.rank + 1}</span>
				<span>{describe(s.row)}</span>
			</li>
		{/each}
	</ul>
	{#if shown.length === 0}
		<p class="muted">Кривите се появяват, щом устройствата завършат първите обучения.</p>
	{:else}
		<div class="plot" bind:clientWidth={width}>
			<svg
				viewBox="0 0 {W} {H}"
				role="img"
				aria-label="Загуба при обучение на петте най-добри настройки"
				onpointermove={move}
				onpointerleave={() => (hover = null)}
			>
				{#each ticks as t, i (i)}
					<line class="grid" x1={PAD.left} x2={W - PAD.right} y1={y(t)} y2={y(t)} />
					<text class="tick" x={PAD.left - 6} y={y(t) + 4} text-anchor="end">{t.toFixed(1)}</text>
				{/each}
				<text class="tick" x={PAD.left} y={H - 8}>начало</text>
				<text class="tick" x={W - PAD.right} y={H - 8} text-anchor="end">край на обучението</text>
				{#if hover !== null}
					<line
						class="cross"
						x1={x(hover, points)}
						x2={x(hover, points)}
						y1={PAD.top}
						y2={H - PAD.bottom}
					/>
				{/if}
				{#each [...shown].reverse() as s (s.row.config)}
					<path class="line" d={path(s.row.curve)} style:--c="var(--s{s.slot + 1})" />
					{#if hover !== null}
						<circle
							class="dot"
							cx={x(hover, s.row.curve.length)}
							cy={y(s.row.curve[hover])}
							r="4"
							style:--c="var(--s{s.slot + 1})"
						/>
					{/if}
				{/each}
			</svg>
			{#if hover !== null}
				<div class="tip" style:left="{Math.min(78, Math.max(22, (x(hover, points) / W) * 100))}%">
					<strong>{Math.round(((hover + 1) / points) * 100)} % от обучението</strong>
					{#each shown as s (s.row.config)}
						<span
							><span class="swatch" style:--c="var(--s{s.slot + 1})"></span> #{s.rank + 1}: {s.row.curve[
								hover
							].toFixed(3)}</span
						>
					{/each}
				</div>
			{/if}
		</div>
		<p class="muted small">Загуба при обучение: колкото по-ниско, толкова по-добре.</p>
	{/if}
</div>

<style>
	/* Categorical slots 1–5 of the validated reference palette (dataviz). */
	.curves {
		--s1: #2a78d6;
		--s2: #eb6834;
		--s3: #1baf7a;
		--s4: #eda100;
		--s5: #e87ba4;
		display: grid;
		gap: var(--s-3);
	}
	@media (prefers-color-scheme: dark) {
		.curves {
			--s1: #3987e5;
			--s2: #d95926;
			--s3: #199e70;
			--s4: #c98500;
			--s5: #d55181;
		}
	}
	.legend {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: 4px;
		font-size: var(--text-xs);
	}
	.legend li {
		display: flex;
		align-items: center;
		gap: 8px;
		min-width: 0;
		overflow-wrap: anywhere;
	}
	.rank {
		flex: none;
		font-weight: 700;
		font-variant-numeric: tabular-nums;
	}
	.swatch {
		display: inline-block;
		flex: none;
		width: 12px;
		height: 3px;
		border-radius: 2px;
		background: var(--c);
	}
	.plot {
		position: relative;
	}
	svg {
		display: block;
		width: 100%;
		height: auto;
		touch-action: pan-y;
	}
	.grid {
		stroke: var(--border);
		stroke-width: 1;
	}
	.tick {
		fill: var(--muted);
		font-size: 11px;
	}
	.line {
		fill: none;
		stroke: var(--c);
		stroke-width: 2;
		stroke-linejoin: round;
		stroke-linecap: round;
	}
	.cross {
		stroke: var(--muted);
		stroke-width: 1;
		stroke-dasharray: 3 3;
	}
	.dot {
		fill: var(--c);
		stroke: var(--bg);
		stroke-width: 2;
	}
	.tip {
		position: absolute;
		top: 0;
		transform: translateX(-50%);
		display: grid;
		gap: 2px;
		padding: 8px 10px;
		border-radius: var(--radius);
		background: var(--glass-strong);
		border: 1px solid var(--glass-border);
		backdrop-filter: blur(12px);
		font-size: var(--text-xs);
		font-variant-numeric: tabular-nums;
		pointer-events: none;
		white-space: nowrap;
	}
	.small {
		font-size: var(--text-xs);
		margin: 0;
	}
</style>
