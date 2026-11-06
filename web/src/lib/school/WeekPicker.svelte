<script lang="ts">
	// A week's grid of periods. For a teacher or class (`away`), the hours they are away:
	// click a period to mark it, again to clear it, or a day's name for the whole day. For
	// a lesson (`pin`, spec 0015), the slots its hours are pinned at: a slot the lesson
	// cannot have (its teacher away, another pinned hour) is shown with why, and only a
	// slot pinned already can be cleared there.
	import { DAY_NAMES } from '#lib/console/timetable.ts';
	import type { TtSlot } from '#lib/protocol/TtSlot.ts';

	let {
		days,
		periods,
		marked,
		ontoggle,
		label,
		mode = 'away',
		blocked = []
	}: {
		days: number;
		periods: number;
		marked: TtSlot[];
		ontoggle: (slots: TtSlot[]) => void;
		label: string;
		mode?: 'away' | 'pin';
		blocked?: (TtSlot & { why: string })[];
	} = $props();

	const isMarked = (day: number, period: number) =>
		marked.some((s) => s.day === day && s.period === period);
	const whyNot = (day: number, period: number) =>
		blocked.find((s) => s.day === day && s.period === period)?.why;

	/** A whole day: marks it all, or clears it when it is all marked already. */
	function day(d: number) {
		const all = Array.from({ length: periods }, (_, period) => ({ day: d, period }));
		const allMarked = all.every((s) => isMarked(s.day, s.period));
		ontoggle(all.filter((s) => isMarked(s.day, s.period) === allMarked));
	}

	function state(d: number, p: number) {
		if (mode === 'away') return isMarked(d, p) ? 'отсъства' : 'на разположение';
		if (isMarked(d, p)) return 'закрепен';
		return whyNot(d, p) ?? 'свободен';
	}
</script>

<div class="table-wrap">
	<table class="picker" class:pins={mode === 'pin'} aria-label={label}>
		<thead>
			<tr>
				<th scope="col">Час</th>
				{#each Array.from({ length: days }, (_, d) => d) as d (d)}
					<th scope="col">
						{#if mode === 'away'}
							<button
								type="button"
								class="ghost sm"
								onclick={() => day(d)}
								data-testid="away-day-{d}">{DAY_NAMES[d] ?? d + 1}</button
							>
						{:else}
							{DAY_NAMES[d] ?? d + 1}
						{/if}
					</th>
				{/each}
			</tr>
		</thead>
		<tbody>
			{#each Array.from({ length: periods }, (_, p) => p) as p (p)}
				<tr>
					<th scope="row">{p + 1}</th>
					{#each Array.from({ length: days }, (_, d) => d) as d (d)}
						{@const why = mode === 'pin' && !isMarked(d, p) ? whyNot(d, p) : undefined}
						<td>
							<button
								type="button"
								class="slot"
								aria-pressed={isMarked(d, p)}
								aria-label="{DAY_NAMES[d] ?? d + 1}, {p + 1} час: {state(d, p)}"
								title={why}
								disabled={why !== undefined}
								data-testid="{mode}-{d}-{p}"
								onclick={() => ontoggle([{ day: d, period: p }])}
								>{isMarked(d, p) ? (mode === 'away' ? 'няма' : '📌') : why ? '–' : ''}</button
							>
						</td>
					{/each}
				</tr>
			{/each}
		</tbody>
	</table>
</div>

<style>
	.picker {
		table-layout: fixed;
		min-width: 520px;
	}
	.picker th[scope='row'] {
		width: 3rem;
		text-align: center;
	}
	.picker td {
		padding: 3px;
	}
	.slot {
		width: 100%;
		min-height: 2.2rem;
		padding: 0;
		background: rgb(118 118 128 / 0.06);
		color: var(--text);
		border: 1px solid var(--border);
		box-shadow: none;
		font-size: var(--text-sm);
	}
	.slot:hover:not(:disabled) {
		background: rgb(118 118 128 / 0.14);
	}
	.slot:disabled {
		color: var(--muted);
		cursor: not-allowed;
		background: repeating-linear-gradient(
			135deg,
			transparent 0 6px,
			rgb(118 118 128 / 0.1) 6px 9px
		);
	}
	.slot[aria-pressed='true'] {
		background: color-mix(in srgb, var(--bad) 22%, transparent);
		border-color: color-mix(in srgb, var(--bad) 45%, transparent);
		color: var(--bad);
	}
	.pins .slot[aria-pressed='true'] {
		background: color-mix(in srgb, var(--brand) 18%, transparent);
		border-color: color-mix(in srgb, var(--brand) 50%, transparent);
		color: var(--text);
	}
</style>
