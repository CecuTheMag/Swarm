<script lang="ts">
	// Timetables for the staff room's notice board (spec 0014): one A4 page per class or
	// teacher. Shown only when printing; the rest of the page is left off the paper.
	import TimetableGrid from './TimetableGrid.svelte';
	import { grid, type Placed, type View } from './timetable.ts';
	import type { TtSchool } from '#lib/protocol/TtSchool.ts';

	let { school, placed, sheets }: { school: TtSchool; placed: Placed[]; sheets: View[] } = $props();

	/** Out of the page's layout, straight under `body`, so print rules can keep it alone. */
	function portal(node: HTMLElement) {
		document.body.appendChild(node);
		return { destroy: () => node.remove() };
	}

	const printed = new Date().toLocaleDateString('bg-BG');
</script>

<div class="print-root" use:portal data-testid="print-sheets">
	{#each sheets as view (view.kind + view.index)}
		<section class="sheet" data-testid="print-sheet">
			<header>
				<span>{school.name}</span>
				<h1>
					{view.kind === 'class'
						? `Паралелка ${school.classes[view.index].name}`
						: school.teachers[view.index].name}
				</h1>
			</header>
			<TimetableGrid cells={grid(school, placed, view)} days={school.days} kind={view.kind} />
			<footer>Отпечатано на {printed} · Рояк</footer>
		</section>
	{/each}
</div>

<style>
	.sheet {
		break-after: page;
		color: #000;
	}
	.sheet:last-child {
		break-after: auto;
	}
	header {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		gap: 1rem;
		margin-bottom: 0.5rem;
		font-size: 0.85rem;
	}
	h1 {
		margin: 0;
		font-size: 1.4rem;
		order: -1;
	}
	footer {
		margin-top: 0.5rem;
		font-size: 0.75rem;
		color: #444;
	}
	.sheet :global(.table-wrap) {
		overflow: visible;
	}
	.sheet :global(table) {
		width: 100%;
		min-width: 0;
		border-collapse: collapse;
		color: #000;
	}
	.sheet :global(th),
	.sheet :global(td) {
		border: 1px solid #999;
		background: none;
	}
	.sheet :global(.muted) {
		color: #333;
	}
</style>
