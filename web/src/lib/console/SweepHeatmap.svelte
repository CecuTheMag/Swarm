<script lang="ts">
	import type { NnRow } from '#lib/protocol/NnRow.ts';

	// Accuracy over learning rate × layer size (spec 0011): each cell is the best of
	// the configurations with those two settings. One hue, light to dark.
	let {
		rows,
		learningRates,
		hidden
	}: { rows: NnRow[]; learningRates: number[]; hidden: number[] } = $props();

	const RAMP = [
		'#cde2fb',
		'#b7d3f6',
		'#9ec5f4',
		'#86b6ef',
		'#6da7ec',
		'#5598e7',
		'#3987e5',
		'#2a78d6',
		'#256abf',
		'#1c5cab',
		'#184f95',
		'#104281',
		'#0d366b'
	];

	const cells = $derived.by(() => {
		const best: Record<string, NnRow> = {};
		for (const r of rows) {
			const key = `${r.hidden}:${r.learning_rate}`;
			if (!best[key] || r.accuracy > best[key].accuracy) best[key] = r;
		}
		return best;
	});
	const range = $derived.by(() => {
		const values = Object.values(cells).map((r) => r.accuracy);
		return values.length ? { lo: Math.min(...values), hi: Math.max(...values) } : null;
	});
	/** Ramp step of an accuracy, relative to the cells shown. */
	const step = (a: number) => {
		if (!range || range.hi === range.lo) return RAMP.length - 1;
		return Math.round(((a - range.lo) / (range.hi - range.lo)) * (RAMP.length - 1));
	};
	const pct = (a: number) => `${(a * 100).toFixed(1)} %`;
	const sortedLr = $derived([...learningRates].sort((a, b) => a - b));
	const sortedHidden = $derived([...hidden].sort((a, b) => b - a));
</script>

<div class="heat" style:--cols={sortedLr.length}>
	<div class="grid" role="table" aria-label="Точност по скорост на учене и неврони в слой">
		<div class="corner" role="row">
			<span class="axis-y" role="columnheader">неврони</span>
			{#each sortedLr as lr (lr)}
				<span class="col-head" role="columnheader">{lr}</span>
			{/each}
		</div>
		{#each sortedHidden as h (h)}
			<div class="row" role="row">
				<span class="row-head" role="rowheader">{h}</span>
				{#each sortedLr as lr (lr)}
					{@const cell = cells[`${h}:${lr}`]}
					{#if cell}
						{@const s = step(cell.accuracy)}
						<span
							class="cell"
							class:high={s >= 7}
							style:--fill={RAMP[s]}
							style:--fill-dark={RAMP[RAMP.length - 1 - s]}
							role="cell"
							tabindex="0"
							title="{h} неврона, скорост {lr}: {pct(cell.accuracy)} ({cell.layers} {cell.layers ===
							1
								? 'слой'
								: 'слоя'}, партида {cell.batch})"
							data-testid="heat-cell">{(cell.accuracy * 100).toFixed(0)}</span
						>
					{:else}
						<span class="cell empty" role="cell" title="{h} неврона, скорост {lr}: още се обучава"
							>…</span
						>
					{/if}
				{/each}
			</div>
		{/each}
	</div>
	<div class="axis-x">скорост на учене →</div>
	{#if range}
		<div class="legend" aria-hidden="true">
			<span>{pct(range.lo)}</span>
			<span class="bar"></span>
			<span>{pct(range.hi)}</span>
		</div>
	{/if}
</div>

<style>
	.heat {
		display: grid;
		gap: var(--s-2);
		font-size: var(--text-xs);
	}
	.grid {
		display: grid;
		gap: 2px;
	}
	.corner,
	.row {
		display: grid;
		grid-template-columns: 64px repeat(var(--cols), minmax(0, 1fr));
		gap: 2px;
		align-items: center;
	}
	.axis-y,
	.row-head,
	.col-head {
		color: var(--muted);
		font-variant-numeric: tabular-nums;
	}
	.col-head {
		text-align: center;
	}
	.row-head {
		text-align: right;
		padding-right: 8px;
	}
	.cell {
		display: grid;
		place-items: center;
		min-height: 36px;
		border-radius: 4px;
		background: var(--fill);
		color: #1a1a19;
		font-weight: 600;
		font-variant-numeric: tabular-nums;
		transition:
			background-color var(--slow) var(--ease),
			transform var(--fast) var(--ease);
		outline-offset: 2px;
	}
	/* High values are the darkest steps on light, the lightest on dark. */
	.cell.high {
		color: #fff;
	}
	.cell:hover,
	.cell:focus-visible {
		transform: scale(1.06);
	}
	.cell.empty {
		background: rgb(118 118 128 / 0.1);
		color: var(--muted);
		font-weight: 400;
	}
	.axis-x {
		color: var(--muted);
		text-align: right;
	}
	.legend {
		display: flex;
		align-items: center;
		gap: 8px;
		color: var(--muted);
		font-variant-numeric: tabular-nums;
	}
	.bar {
		flex: 1;
		height: 8px;
		border-radius: 4px;
		background: linear-gradient(90deg, #cde2fb, #3987e5, #0d366b);
	}
	@media (prefers-color-scheme: dark) {
		/* Dark surface: low accuracy recedes into it, high accuracy is the lightest. */
		.cell {
			background: var(--fill-dark);
			color: #f0efec;
		}
		.cell.high {
			color: #1a1a19;
		}
		.bar {
			background: linear-gradient(90deg, #0d366b, #3987e5, #cde2fb);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.cell {
			transition: none;
		}
	}
</style>
