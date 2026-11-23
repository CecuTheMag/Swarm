<script lang="ts">
	import { fade } from 'svelte/transition';
	import { deviceLabel, type Caught } from './state.ts';
	import type { WorkerSummary } from '#lib/protocol/WorkerSummary.ts';

	// Every device in the swarm as a cell of a honeycomb: it appears when the device
	// joins, flashes when the device finishes work, and turns red when it is caught
	// returning wrong results. Empty cells show how many more the room could add.

	let { workers, caught }: { workers: WorkerSummary[]; caught: Caught[] } = $props();

	// A device keeps its cell while it is shown, so others do not move when one leaves.
	// Plain bookkeeping read only inside `cells`, which recomputes on every change anyway.
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	const slots = new Map<string, number>();

	const cells = $derived.by(() => {
		const shown: Record<string, { worker: WorkerSummary; caught: Caught | null }> = {};
		for (const worker of workers) shown[worker.id] = { worker, caught: null };
		for (const c of caught) {
			shown[c.worker.id] = { worker: shown[c.worker.id]?.worker ?? c.worker, caught: c };
		}
		for (const id of [...slots.keys()]) if (!(id in shown)) slots.delete(id);
		const used: Record<number, true> = {};
		for (const slot of slots.values()) used[slot] = true;
		const ordered = Object.values(shown).sort(
			(a, b) => a.worker.connected_at - b.worker.connected_at
		);
		let free = 0;
		for (const { worker } of ordered) {
			if (slots.has(worker.id)) continue;
			while (used[free]) free++;
			slots.set(worker.id, free);
			used[free] = true;
		}
		return ordered.map((c) => ({ ...c, slot: slots.get(c.worker.id)! }));
	});

	// Room for the next devices; grows in steps so the honeycomb does not reflow on
	// every join.
	const capacity = $derived.by(() => {
		const needed = Math.max(cells.length, ...cells.map((c) => c.slot + 1));
		let size = 12;
		while (size < needed) size *= 2;
		return size;
	});
	// About 4:3, wider than tall.
	const cols = $derived(Math.max(4, Math.ceil(Math.sqrt(capacity * 1.6))));
	const rows = $derived(Math.ceil(capacity / cols));

	const W = Math.sqrt(3);
	const centre = (slot: number) => {
		const row = Math.floor(slot / cols);
		const col = slot % cols;
		return [W * (col + 0.5 + (row % 2) / 2), 1 + 1.5 * row] as const;
	};
	const hex = (x: number, y: number, r = 0.92) =>
		Array.from({ length: 6 }, (_, k) => {
			const a = (Math.PI / 3) * k + Math.PI / 6;
			return `${(x + r * Math.cos(a)).toFixed(3)},${(y + r * Math.sin(a)).toFixed(3)}`;
		}).join(' ');

	// Every free cell as one path: a thousand outlines cost one element.
	const empty = $derived.by(() => {
		const taken: Record<number, true> = {};
		for (const c of cells) taken[c.slot] = true;
		let d = '';
		for (let slot = 0; slot < capacity; slot++) {
			if (taken[slot]) continue;
			const [x, y] = centre(slot);
			d += `M${hex(x, y).replaceAll(' ', 'L')}Z`;
		}
		return d;
	});

	const counts = $derived({
		gpu: workers.filter((w) => w.device.backend === 'webgpu').length,
		cpu: workers.filter((w) => w.device.backend === 'wasm').length,
		server: workers.filter((w) => w.device.backend === 'native').length
	});
	const kind = (w: WorkerSummary) =>
		w.device.backend === 'webgpu' ? 'gpu' : w.device.backend === 'wasm' ? 'cpu' : 'server';
</script>

<figure class="swarm" data-testid="swarm-map">
	<svg
		viewBox="0 0 {W * (cols + 0.5)} {1.5 * rows + 0.5}"
		role="img"
		aria-label="{workers.length} устройства: {counts.gpu} с видеокарта, {counts.cpu} с процесор, {counts.server} сървърни"
	>
		<path class="free" d={empty} />
		{#each cells as { worker, caught: bad, slot } (worker.id)}
			{@const [x, y] = centre(slot)}
			<g
				class="cell {kind(worker)}"
				class:caught={bad}
				class:gone={bad?.left_at}
				data-testid="swarm-cell"
				data-worker={worker.id}
				data-caught={bad ? 'true' : undefined}
				out:fade={{ duration: 600 }}
			>
				<title>{deviceLabel(worker.device)} · {worker.units_done} единици</title>
				<polygon points={hex(x, y)} />
				{#if bad}
					<path
						class="cross"
						d="M{x - 0.32} {y - 0.32}L{x + 0.32} {y + 0.32}M{x + 0.32} {y - 0.32}L{x - 0.32} {y +
							0.32}"
					/>
				{:else}
					<!-- Re-created, so it flashes again, whenever the device finishes work. -->
					{#key worker.units_done}
						<polygon class="flash" points={hex(x, y)} />
					{/key}
				{/if}
			</g>
		{/each}
	</svg>
	<figcaption>
		<span class="key gpu">видеокарта {counts.gpu}</span>
		<span class="key cpu">процесор {counts.cpu}</span>
		{#if counts.server > 0}<span class="key server">сървър {counts.server}</span>{/if}
		{#if caught.length > 0}<span class="key caught">хванат в измама {caught.length}</span>{/if}
	</figcaption>
</figure>

<style>
	/* Device kinds by amber weight, as on the home page's field: red stays for status. */
	.swarm {
		--gpu: var(--brand);
		--cpu: color-mix(in srgb, var(--brand) 38%, transparent);
		--server: color-mix(in srgb, var(--muted) 34%, transparent);
		margin: 0;
	}
	svg {
		display: block;
		width: 100%;
		height: auto;
		max-height: 100%;
	}
	.free {
		fill: none;
		stroke: var(--border);
		stroke-width: 0.06;
	}
	.cell polygon {
		transform-box: fill-box;
		transform-origin: center;
	}
	.cell > polygon:first-of-type {
		animation: appear var(--slow) var(--ease);
	}
	.gpu polygon {
		fill: var(--gpu);
	}
	.cpu polygon {
		fill: var(--cpu);
		stroke: var(--brand);
		stroke-width: 0.05;
	}
	.server polygon {
		fill: var(--server);
		stroke: color-mix(in srgb, var(--muted) 70%, transparent);
		stroke-width: 0.05;
	}
	.flash {
		fill: #fff4dc !important;
		stroke: none !important;
		opacity: 0;
		animation: flash 0.9s var(--ease);
	}
	.caught polygon {
		fill: var(--bad);
		stroke: none;
		animation: caught 0.8s var(--ease-in-out) 3;
	}
	.caught.gone {
		opacity: 0.45;
	}
	.cross {
		stroke: var(--on-bad);
		stroke-width: 0.14;
		stroke-linecap: round;
		pointer-events: none;
	}
	figcaption {
		display: flex;
		flex-wrap: wrap;
		gap: 4px 14px;
		margin-top: 8px;
		color: var(--muted);
		font-size: 0.875rem;
	}
	.key::before {
		content: '';
		display: inline-block;
		width: 0.7em;
		height: 0.7em;
		margin-right: 0.35em;
		border-radius: 2px;
		background: var(--c);
	}
	.key.gpu {
		--c: var(--gpu);
	}
	.key.cpu {
		--c: var(--cpu);
	}
	.key.cpu::before {
		box-shadow: inset 0 0 0 1px var(--brand);
	}
	.key.server {
		--c: var(--server);
	}
	.key.caught {
		--c: var(--bad);
		color: var(--bad);
		font-weight: 700;
	}
	@keyframes appear {
		from {
			opacity: 0;
			transform: scale(0.7);
		}
	}
	@keyframes flash {
		from {
			opacity: 0.75;
		}
		to {
			opacity: 0;
		}
	}
	@keyframes caught {
		0%,
		100% {
			transform: scale(1);
		}
		50% {
			transform: scale(0.86);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.cell > polygon:first-of-type,
		.flash,
		.caught polygon {
			animation: none;
		}
	}
</style>
