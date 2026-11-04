<script lang="ts">
	import { api, failureText } from '#lib/api.ts';
	import {
		networkOf,
		PENS,
		rasterize,
		SIDE,
		type Model,
		type Point,
		type Reading
	} from './digit.ts';

	// Try the sweep's best network (spec 0011): draw a digit and watch the network the
	// room trained read it, as you draw.
	let { url, dataset }: { url: string; dataset: string } = $props();
	const pen = $derived(PENS[dataset] ?? PENS.glyphs);

	let read = $state<((pixels: Uint8Array) => Reading) | null>(null);
	let error = $state<string | null>(null);
	$effect(() => {
		read = null;
		error = null;
		// The coordinator retrains the network for the download: a few seconds.
		api<Model>(url)
			.then((model) => (read = networkOf(model)))
			.catch((e) => (error = failureText(e)));
	});

	let pad = $state<HTMLCanvasElement>();
	let preview = $state<HTMLCanvasElement>();
	let strokes = $state<Point[][]>([]);
	let drawing = false;
	let reading = $state<Reading | null>(null);
	let frame = 0;

	/** Pen width on the pad, in pad pixels: about the dataset's pen at 14 pixels. */
	const PEN = 0.075;

	function point(e: PointerEvent): Point {
		const r = pad!.getBoundingClientRect();
		return { x: (e.clientX - r.left) / r.width, y: (e.clientY - r.top) / r.height };
	}

	function down(e: PointerEvent) {
		if (!read) return;
		pad!.setPointerCapture(e.pointerId);
		drawing = true;
		strokes.push([point(e)]);
		update();
	}
	function move(e: PointerEvent) {
		if (!drawing) return;
		strokes[strokes.length - 1].push(point(e));
		update();
	}
	function up() {
		drawing = false;
	}
	function clear() {
		strokes = [];
		reading = null;
		update();
	}

	// Redraws and rereads once per frame, however fast the pointer moves.
	function update() {
		if (frame) return;
		frame = requestAnimationFrame(() => {
			frame = 0;
			const pixels = rasterize(strokes, pen);
			if (strokes.length > 0 && read) reading = read(pixels);
			paint(pixels);
		});
	}

	function paint(pixels: Uint8Array) {
		if (!pad || !preview) return;
		const ink = getComputedStyle(pad).color;
		const size = pad.clientWidth * devicePixelRatio;
		pad.width = pad.height = size;
		const g = pad.getContext('2d')!;
		g.strokeStyle = ink;
		g.fillStyle = ink;
		g.lineCap = g.lineJoin = 'round';
		g.lineWidth = PEN * size;
		for (const s of strokes) {
			g.beginPath();
			g.moveTo(s[0].x * size, s[0].y * size);
			if (s.length === 1) g.lineTo(s[0].x * size + 0.1, s[0].y * size);
			for (const p of s.slice(1)) g.lineTo(p.x * size, p.y * size);
			g.stroke();
		}
		const v = preview.getContext('2d')!;
		const image = v.createImageData(SIDE, SIDE);
		pixels.forEach((p, i) => {
			image.data.set([255 - p, 255 - p, 255 - p, 255], i * 4);
		});
		v.putImageData(image, 0, 0);
	}
	$effect(() => {
		if (pad && preview) paint(rasterize(strokes, pen));
	});

	const ranked = $derived(reading?.probabilities ?? Array<number>(10).fill(0));
	const sure = $derived(reading ? reading.probabilities[reading.digit] : 0);
</script>

<div class="try" data-testid="digit-pad" data-ready={read !== null}>
	<div class="drawing">
		<canvas
			bind:this={pad}
			class="pad"
			class:off={!read}
			aria-label="Поле за рисуване на цифра"
			onpointerdown={down}
			onpointermove={move}
			onpointerup={up}
			onpointercancel={up}
		></canvas>
		<div class="under">
			<span class="muted small">
				{#if error}
					<span class="bad">{error}</span>
				{:else if !read}
					Зареждам мрежата…
				{:else}
					Нарисувайте една цифра от 0 до 9.
				{/if}
			</span>
			<button class="button" onclick={clear} disabled={strokes.length === 0}>Изчисти</button>
		</div>
	</div>
	<div class="answer" aria-live="polite">
		<div class="guess">
			<div class="digit" data-testid="digit-guess">{reading ? reading.digit : '–'}</div>
			<div>
				<div class="what">{reading ? 'Мрежата казва' : 'Чака рисунка'}</div>
				<div class="muted small">
					{reading ? `сигурна на ${Math.round(sure * 100)} %` : 'познава, докато рисувате'}
				</div>
			</div>
			<figure>
				<canvas bind:this={preview} class="preview" width={SIDE} height={SIDE}></canvas>
				<figcaption class="muted">как я вижда</figcaption>
			</figure>
		</div>
		<ol class="bars" aria-label="Колко е сигурна мрежата за всяка цифра">
			{#each ranked as p, d (d)}
				<li class:top={reading?.digit === d}>
					<span class="label">{d}</span>
					<span class="track"><span class="bar" style:width="{p * 100}%"></span></span>
					<span class="value">{Math.round(p * 100)} %</span>
				</li>
			{/each}
		</ol>
	</div>
</div>

<style>
	.try {
		display: grid;
		grid-template-columns: minmax(0, 300px) minmax(0, 1fr);
		gap: var(--s-5);
		align-items: start;
	}
	.pad {
		display: block;
		width: 100%;
		aspect-ratio: 1;
		border-radius: var(--radius);
		border: 1px solid var(--border-strong);
		background: var(--bg-elevated);
		color: var(--text);
		touch-action: none;
		cursor: crosshair;
	}
	.pad.off {
		cursor: progress;
		opacity: 0.6;
	}
	.under {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--s-2);
		margin-top: var(--s-2);
	}
	.guess {
		display: flex;
		align-items: center;
		gap: var(--s-4);
		margin-bottom: var(--s-3);
	}
	.digit {
		font-size: clamp(3rem, 10vw, 4.5rem);
		font-weight: 750;
		line-height: 1;
		width: 1.2em;
		text-align: center;
		font-variant-numeric: tabular-nums;
	}
	.what {
		font-weight: 600;
	}
	figure {
		margin: 0 0 0 auto;
		text-align: center;
	}
	.preview {
		width: 56px;
		height: 56px;
		image-rendering: pixelated;
		border-radius: var(--radius-sm);
		border: 1px solid var(--border);
	}
	figcaption {
		font-size: var(--text-xs);
	}
	.bars {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: 6px;
	}
	.bars li {
		display: grid;
		grid-template-columns: 1.2em minmax(0, 1fr) 3.2em;
		align-items: center;
		gap: var(--s-2);
		font-size: var(--text-sm);
		font-variant-numeric: tabular-nums;
	}
	.label {
		font-weight: 600;
		text-align: center;
	}
	.value {
		text-align: right;
		color: var(--muted);
	}
	.track {
		height: 10px;
		border-radius: 5px;
		background: var(--border);
		overflow: hidden;
	}
	.bar {
		display: block;
		height: 100%;
		border-radius: 5px;
		background: var(--faint);
		transition: width var(--fast) var(--ease);
	}
	.top .bar {
		background: var(--brand);
	}
	.top .value {
		color: var(--text);
		font-weight: 600;
	}
	@media (max-width: 640px) {
		.try {
			grid-template-columns: minmax(0, 1fr);
		}
		.drawing {
			max-width: 320px;
			width: 100%;
			margin: 0 auto;
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.bar {
			transition: none;
		}
	}
</style>
