<script lang="ts">
	// The hero's honeycomb: cells light up as if devices were finishing work. The pattern
	// is fixed (no Math.random) so server and browser render the same markup.

	const COLS = 9;
	const ROWS = 6;
	const W = Math.sqrt(3);

	const hex = (x: number, y: number, r = 0.9) =>
		Array.from({ length: 6 }, (_, k) => {
			const a = (Math.PI / 3) * k + Math.PI / 6;
			return `${(x + r * Math.cos(a)).toFixed(3)},${(y + r * Math.sin(a)).toFixed(3)}`;
		}).join(' ');

	const cells = Array.from({ length: COLS * ROWS }, (_, i) => {
		const row = Math.floor(i / COLS);
		const col = i % COLS;
		const x = W * (col + 0.5 + (row % 2) / 2);
		const y = 1 + 1.5 * row;
		// A cheap, stable hash picks which cells take part and when they flash.
		const h = (i * 2654435761) >>> 0;
		const kind = h % 7 === 0 ? 'cpu' : h % 3 === 0 ? 'idle' : 'gpu';
		return { points: hex(x, y), kind, delay: (h % 4000) / 1000, period: 3 + (h % 5) };
	});
	const width = W * (COLS + 0.5);
	const height = 1.5 * ROWS + 0.5;
</script>

<svg
	viewBox="0 0 {width} {height}"
	class="hive"
	aria-hidden="true"
	preserveAspectRatio="xMidYMid slice"
>
	{#each cells as c, i (i)}
		<polygon
			points={c.points}
			class={c.kind}
			style:--delay="{c.delay}s"
			style:--period="{c.period}s"
		/>
	{/each}
</svg>

<style>
	.hive {
		width: 100%;
		height: 100%;
		display: block;
	}
	polygon {
		fill: rgb(118 118 128 / 0.08);
		stroke: rgb(118 118 128 / 0.16);
		stroke-width: 0.03;
	}
	.gpu {
		fill: var(--brand);
		opacity: 0.18;
		animation: work var(--period) var(--ease-in-out) var(--delay) infinite;
	}
	.cpu {
		fill: #5b8cff;
		opacity: 0.16;
		animation: work var(--period) var(--ease-in-out) var(--delay) infinite;
	}
	@keyframes work {
		0%,
		70%,
		100% {
			opacity: 0.18;
		}
		80% {
			opacity: 0.95;
		}
	}
</style>
