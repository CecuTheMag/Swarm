<script lang="ts">
	// One class's or teacher's week: periods down, days across.
	import { DAY_NAMES, type Placed } from './timetable.ts';

	let { cells, days, kind }: { cells: Placed[][][]; days: number; kind: 'class' | 'teacher' } =
		$props();
</script>

<div class="table-wrap">
	<table class="week" data-testid="timetable-grid">
		<thead>
			<tr>
				<th scope="col">Час</th>
				{#each Array.from({ length: days }, (_, d) => d) as d (d)}
					<th scope="col">{DAY_NAMES[d] ?? d + 1}</th>
				{/each}
			</tr>
		</thead>
		<tbody>
			{#each cells as row, period (period)}
				<tr>
					<th scope="row">{period + 1}</th>
					{#each row as hours, day (day)}
						<td class:clash={hours.length > 1} class:empty={hours.length === 0}>
							{#each hours as h (h.lesson + ':' + h.day + ':' + h.period + ':' + h.class)}
								<div class="hour" data-testid="hour">
									<strong>{h.subject}</strong>
									<span class="muted">{kind === 'class' ? h.teacher : h.class}</span>
									{#if h.room}<span class="muted room">{h.room}</span>{/if}
								</div>
							{/each}
						</td>
					{/each}
				</tr>
			{/each}
		</tbody>
	</table>
</div>

<style>
	.week {
		table-layout: fixed;
		min-width: 640px;
	}
	.week th[scope='row'] {
		width: 3rem;
		text-align: center;
	}
	.week td {
		vertical-align: top;
		padding: 6px;
		height: 4.2rem;
	}
	.week td.empty {
		background: rgb(118 118 128 / 0.05);
	}
	.week td.clash {
		background: color-mix(in srgb, var(--bad) 14%, transparent);
	}
	.hour {
		display: grid;
		gap: 2px;
		font-size: var(--text-sm);
		line-height: 1.25;
	}
	.hour + .hour {
		margin-top: 6px;
		padding-top: 6px;
		border-top: 1px dashed var(--border);
	}
	.room {
		font-size: 0.75rem;
	}
</style>
