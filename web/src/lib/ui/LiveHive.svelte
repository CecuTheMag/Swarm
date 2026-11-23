<script lang="ts">
	import { onMount } from 'svelte';
	import { HiveModel, layout, type Kind, type Layout } from './hive-field.ts';
	import type { WorkerSummary } from '#lib/protocol/WorkerSummary.ts';

	// The home page's live honeycomb: the real network, one cell per device (see
	// hive-field.ts for what each state means). One canvas, drawn only while something
	// changes, at most 30 frames a second, and not at all while the tab is hidden or
	// the field is off screen. Decorative: the numbers beside it carry the same facts.

	let { workers, caught }: { workers: WorkerSummary[]; caught: ReadonlySet<string> } = $props();

	let canvas: HTMLCanvasElement;
	let host: HTMLDivElement;
	/** Average milliseconds per drawn frame, for checking the budget (data-frame-ms,
	 * written at most once a second so measuring does not itself touch the DOM). */
	let frameMs = 0;
	let reported = 0;

	const model = new HiveModel();
	let field: Layout = { r: 0, slots: [] };
	let width = 0;
	let height = 0;
	let dpr = 1;

	interface Palette {
		empty: string;
		emptyLine: string;
		kind: Record<Kind, string>;
		line: Record<Kind, string>;
		flash: string;
		bad: string;
	}
	let palette: Palette | null = null;
	/** The empty honeycomb, drawn once per size and theme, copied under every frame. */
	let base: HTMLCanvasElement | null = null;

	/** `#rrggbb` at `alpha`: canvas colours, without relying on color-mix() in canvas. */
	function rgba(hex: string, alpha: number) {
		const n = parseInt(hex.replace('#', ''), 16);
		return `rgb(${(n >> 16) & 255} ${(n >> 8) & 255} ${n & 255} / ${alpha})`;
	}

	function readPalette(): Palette {
		const css = getComputedStyle(host);
		const v = (name: string) => css.getPropertyValue(name).trim();
		const dark = matchMedia('(prefers-color-scheme: dark)').matches;
		const brand = v('--brand');
		const muted = v('--muted');
		return {
			empty: dark ? 'rgb(255 255 255 / 0.025)' : 'rgb(118 118 128 / 0.05)',
			emptyLine: dark ? 'rgb(255 255 255 / 0.07)' : 'rgb(118 118 128 / 0.16)',
			// Told apart by amber weight, not by more hues: solid, light, neutral.
			kind: {
				gpu: brand,
				cpu: rgba(brand, 0.38),
				server: rgba(muted, 0.34)
			},
			line: {
				gpu: brand,
				cpu: brand,
				server: rgba(muted, 0.7)
			},
			flash: dark ? '#fff4dc' : '#ffffff',
			bad: v('--bad')
		};
	}

	function hexPath(ctx: CanvasRenderingContext2D, x: number, y: number, r: number) {
		ctx.beginPath();
		for (let k = 0; k < 6; k++) {
			const a = (Math.PI / 3) * k + Math.PI / 6;
			const px = x + r * Math.cos(a);
			const py = y + r * Math.sin(a);
			if (k === 0) ctx.moveTo(px, py);
			else ctx.lineTo(px, py);
		}
		ctx.closePath();
	}

	function relayout() {
		const rect = host.getBoundingClientRect();
		width = rect.width;
		height = rect.height;
		if (width === 0 || height === 0) return;
		dpr = Math.min(2, window.devicePixelRatio || 1);
		canvas.width = Math.round(width * dpr);
		canvas.height = Math.round(height * dpr);
		// Cells about 1/16 of the field's width; smaller only when the network outgrows it.
		const r = Math.max(12, Math.min(26, width / 16));
		// Centred a little high: the figures card covers the bottom of the field.
		field = layout(width, height, Math.max(model.size, workers.length), r, {
			x: width / 2,
			y: height * 0.4
		});
		model.setCapacity(field.slots.length);
		palette = readPalette();
		base = document.createElement('canvas');
		base.width = canvas.width;
		base.height = canvas.height;
		const ctx = base.getContext('2d')!;
		ctx.scale(dpr, dpr);
		ctx.lineWidth = 1;
		for (const { x, y } of field.slots) {
			hexPath(ctx, x, y, field.r * 0.9);
			ctx.fillStyle = palette.empty;
			ctx.fill();
			ctx.strokeStyle = palette.emptyLine;
			ctx.stroke();
		}
		draw(Date.now());
	}

	/** Draws the field at `now`; returns whether it is still changing. */
	function draw(now: number): boolean {
		const ctx = canvas.getContext('2d');
		if (!ctx || !base || !palette) return false;
		const started = performance.now();
		ctx.setTransform(1, 0, 0, 1, 0, 0);
		ctx.clearRect(0, 0, canvas.width, canvas.height);
		ctx.drawImage(base, 0, 0);
		ctx.scale(dpr, dpr);
		ctx.lineWidth = 1;
		for (const cell of model.frame(now)) {
			const at = field.slots[cell.slot];
			if (!at || cell.alpha <= 0) continue;
			const r = field.r * 0.9 * cell.scale;
			hexPath(ctx, at.x, at.y, r);
			// Idle devices stay visible, at a little over a third.
			ctx.globalAlpha = cell.alpha * (0.38 + 0.62 * cell.activity);
			ctx.fillStyle = cell.caught ? palette.bad : palette.kind[cell.kind];
			ctx.fill();
			if (!cell.caught && cell.kind !== 'gpu') {
				ctx.strokeStyle = palette.line[cell.kind];
				ctx.stroke();
			}
			if (cell.flash > 0 && !cell.caught) {
				ctx.globalAlpha = cell.alpha * cell.flash * 0.75;
				ctx.fillStyle = palette.flash;
				ctx.fill();
			}
		}
		ctx.globalAlpha = 1;
		const cost = performance.now() - started;
		frameMs = frameMs === 0 ? cost : frameMs * 0.9 + cost * 0.1;
		if (started - reported > 1000) {
			reported = started;
			host.dataset.frameMs = frameMs.toFixed(2);
		}
		return model.animating(now);
	}

	let running = false;
	let visible = true;
	let onScreen = true;
	let lastFrame = 0;
	let wake: ReturnType<typeof setTimeout> | undefined;

	function schedule() {
		if (running || !visible || !onScreen) return;
		running = true;
		requestAnimationFrame(loop);
	}

	function loop(t: number) {
		if (!visible || !onScreen) {
			running = false;
			return;
		}
		// 30 frames a second at most; 15 if a frame ever costs more than a few ms.
		const interval = frameMs > 4 ? 66 : 33;
		if (t - lastFrame < interval) {
			requestAnimationFrame(loop);
			return;
		}
		lastFrame = t;
		const now = Date.now();
		model.prune(now);
		if (draw(now)) {
			requestAnimationFrame(loop);
			return;
		}
		running = false;
		// Asleep until the next event, or until a device goes idle.
		clearTimeout(wake);
		const next = model.nextChange(now);
		if (next !== null) wake = setTimeout(schedule, next - now + 16);
	}

	// Every state change from the stream.
	$effect(() => {
		const now = Date.now();
		const changed = model.update(workers, caught, now);
		if (!canvas) return;
		if (model.size > field.slots.length) relayout();
		if (changed) {
			if (model.reduced) draw(now);
			schedule();
		}
	});

	onMount(() => {
		const reduce = matchMedia('(prefers-reduced-motion: reduce)');
		const dark = matchMedia('(prefers-color-scheme: dark)');
		model.reduced = reduce.matches;
		const onMotion = () => {
			model.reduced = reduce.matches;
			draw(Date.now());
		};
		const onTheme = () => relayout();
		reduce.addEventListener('change', onMotion);
		dark.addEventListener('change', onTheme);

		const onVisibility = () => {
			visible = document.visibilityState === 'visible';
			if (visible) {
				draw(Date.now());
				schedule();
			}
		};
		document.addEventListener('visibilitychange', onVisibility);
		visible = document.visibilityState === 'visible';

		const resize = new ResizeObserver(() => relayout());
		resize.observe(host);
		const view = new IntersectionObserver(([entry]) => {
			onScreen = entry.isIntersecting;
			if (onScreen) {
				draw(Date.now());
				schedule();
			}
		});
		view.observe(host);

		relayout();
		return () => {
			reduce.removeEventListener('change', onMotion);
			dark.removeEventListener('change', onTheme);
			document.removeEventListener('visibilitychange', onVisibility);
			resize.disconnect();
			view.disconnect();
			clearTimeout(wake);
			visible = false;
		};
	});
</script>

<div class="field" bind:this={host} data-testid="live-hive">
	<canvas bind:this={canvas} aria-hidden="true"></canvas>
</div>

<style>
	.field {
		position: absolute;
		inset: 0;
	}
	canvas {
		display: block;
		width: 100%;
		height: 100%;
	}
</style>
