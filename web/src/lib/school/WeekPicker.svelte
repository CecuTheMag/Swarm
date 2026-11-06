<script lang="ts">
	// The hours of a week a teacher or class is away: click a period to mark it, again
	// to clear it, or a day's name for the whole day.
	import { DAY_NAMES } from '#lib/console/timetable.ts';
	import type { TtSlot } from '#lib/protocol/TtSlot.ts';

	let {
		days,
		periods,
		away,
		ontoggle,
		label
	}: {
		days: number;
		periods: number;
		away: TtSlot[];
		ontoggle: (slots: TtSlot[]) => void;
		label: string;
	} = $props();

	const isAway = (day: number, period: number) =>
		away.some((s) => s.day === day && s.period === period);

	/** A whole day: marks it all, or clears it when it is all marked already. */
	function day(d: number) {
		const all = Array.from({ length: periods }, (_, period) => ({ day: d, period }));
		const allAway = all.every((s) => isAway(s.day, s.period));
		ontoggle(all.filter((s) => isAway(s.day, s.period) === allAway));
	}
</script>

<div class="table-wrap">
	<table class="picker" aria-label={label}>
		<thead>
			<tr>
				<th scope="col">Час</th>
				{#each Array.from({ length: days }, (_, d) => d) as d (d)}
					<th scope="col">
						<button type="button" class="ghost sm" onclick={() => day(d)} data-testid="away-day-{d}"
							>{DAY_NAMES[d] ?? d + 1}</button
						>
					</th>
				{/each}
			</tr>
		</thead>
		<tbody>
			{#each Array.from({ length: periods }, (_, p) => p) as p (p)}
				<tr>
					<th scope="row">{p + 1}</th>
					{#each Array.from({ length: days }, (_, d) => d) as d (d)}
						<td>
							<button
								type="button"
								class="slot"
								aria-pressed={isAway(d, p)}
								aria-label="{DAY_NAMES[d] ?? d + 1}, {p + 1} час: {isAway(d, p)
									? 'отсъства'
									: 'на разположение'}"
								data-testid="away-{d}-{p}"
								onclick={() => ontoggle([{ day: d, period: p }])}
								>{isAway(d, p) ? 'няма' : ''}</button
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
	.slot:hover {
		background: rgb(118 118 128 / 0.14);
	}
	.slot[aria-pressed='true'] {
		background: color-mix(in srgb, var(--bad) 22%, transparent);
		border-color: color-mix(in srgb, var(--bad) 45%, transparent);
		color: var(--bad);
	}
</style>
