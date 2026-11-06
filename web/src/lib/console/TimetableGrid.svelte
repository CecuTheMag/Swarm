<script lang="ts">
	// One class's or teacher's week: periods down, days across. While a timetable is
	// edited (spec 0015), an hour is picked with a click and put with another: every cell
	// then says what putting it there would change.
	import { DAY_NAMES, type Placed } from './timetable.ts';

	/** What putting the picked hour in a cell would change. */
	export interface Preview {
		/** Change in hard violations. */
		hard: number;
		/** Change in the soft cost. */
		soft: number;
	}

	export interface Editing {
		/** The hour picked to be put somewhere, if any. */
		selected: number | null;
		/** Hours pinned by hand, kept by the next search. */
		pins: Set<number>;
		/** Hours away from where the search put them. */
		moved: Set<number>;
		/** `preview[period][day]` for the picked hour; null where it is already. */
		preview: (Preview | null)[][] | null;
		onhour: (hour: number) => void;
		oncell: (day: number, period: number) => void;
	}

	let {
		cells,
		days,
		kind,
		edit = null,
		pins = new Set<number>()
	}: {
		cells: Placed[][][];
		days: number;
		kind: 'class' | 'teacher';
		edit?: Editing | null;
		/** Hours pinned by hand, marked while not editing too. */
		pins?: Set<number>;
	} = $props();

	const pinned = $derived(edit?.pins ?? pins);
	const signed = (n: number) => (n > 0 ? `+${n}` : n < 0 ? `−${-n}` : '±0');
	function badge(p: Preview) {
		return p.hard !== 0 ? `${signed(p.hard)} конфл.` : signed(p.soft);
	}
	const describe = (h: Placed) =>
		`${h.subject}, ${kind === 'class' ? h.teacher : h.class}, ${DAY_NAMES[h.day] ?? h.day + 1} ${h.period + 1}. час`;
</script>

{#snippet hourText(h: Placed)}
	{@const mark = h.pinned ? '🔒' : pinned.has(h.hour) ? '📌' : ''}
	<strong
		>{h.subject}{#if mark}<span
				class="mark"
				title={h.pinned ? 'закрепен в данните на училището' : 'закрепен'}>{mark}</span
			>{/if}</strong
	>
	<span class="muted">{kind === 'class' ? h.teacher : h.class}</span>
	{#if h.room}<span class="muted room">{h.room}</span>{/if}
{/snippet}

<div class="table-wrap">
	<table class="week" class:editing={edit !== null} data-testid="timetable-grid">
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
						{@const preview = edit?.preview?.[period]?.[day] ?? null}
						{@const holdsPicked = hours.some((h) => h.hour === edit?.selected)}
						<td class:clash={hours.length > 1} class:empty={hours.length === 0}>
							{#if edit && edit.selected !== null && !holdsPicked}
								<button
									type="button"
									class="target"
									class:worse={preview !== null && preview.hard > 0}
									class:better={preview !== null &&
										(preview.hard < 0 || (preview.hard === 0 && preview.soft < 0))}
									aria-label="Премести тук: {DAY_NAMES[day] ?? day + 1}, {period + 1}. час{preview
										? `, ${badge(preview)}`
										: ''}"
									data-testid="target"
									data-day={day}
									data-period={period}
									data-hard={preview?.hard}
									data-soft={preview?.soft}
									onclick={() => edit.oncell(day, period)}
								>
									{#each hours as h (h.hour)}
										<span class="hour">{@render hourText(h)}</span>
									{/each}
									{#if preview}<span class="delta">{badge(preview)}</span>{/if}
								</button>
							{:else}
								{#each hours as h (h.hour)}
									{#if edit}
										<button
											type="button"
											class="hour pick"
											class:moved={edit.moved.has(h.hour)}
											aria-pressed={edit.selected === h.hour}
											aria-label="{edit.selected === h.hour ? 'Остави' : 'Избери'} {describe(h)}"
											title={h.pinned ? 'Закрепен в данните на училището: не се мести.' : undefined}
											disabled={h.pinned}
											data-testid="hour"
											data-hour={h.hour}
											onclick={() => edit.onhour(h.hour)}>{@render hourText(h)}</button
										>
									{:else}
										<div class="hour" data-testid="hour" data-hour={h.hour}>
											{@render hourText(h)}
										</div>
									{/if}
								{/each}
							{/if}
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
	.editing td {
		padding: 3px;
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
	.mark {
		margin-left: 0.3em;
		font-weight: 400;
	}
	@media print {
		/* Pins matter to whoever edits the timetable, not to the staff room. */
		.mark {
			display: none;
		}
	}
	button.hour,
	.target {
		width: 100%;
		min-height: 3.6rem;
		padding: 4px 6px;
		text-align: left;
		color: var(--text);
		background: transparent;
		border: 1px solid transparent;
		border-radius: var(--radius-sm);
		box-shadow: none;
		font: inherit;
		/* A cell's width, as the hours shown when not editing: names wrap. */
		display: grid;
		justify-content: stretch;
		align-items: start;
		min-width: 0;
		white-space: normal;
		overflow-wrap: anywhere;
		cursor: pointer;
	}
	button.hour:hover:not(:disabled) {
		border-color: var(--border-strong);
	}
	button.hour:disabled {
		cursor: not-allowed;
		opacity: 1;
	}
	button.hour.moved {
		border-color: color-mix(in srgb, var(--brand) 45%, transparent);
	}
	button.hour[aria-pressed='true'] {
		background: color-mix(in srgb, var(--brand) 18%, transparent);
		border-color: var(--brand);
	}
	.target {
		align-content: start;
		gap: 4px;
		border: 1px dashed var(--border-strong);
	}
	.target:hover {
		background: rgb(118 118 128 / 0.1);
	}
	.target.worse {
		border-color: color-mix(in srgb, var(--bad) 55%, transparent);
	}
	.target.better {
		border-color: color-mix(in srgb, var(--ok) 60%, transparent);
	}
	.delta {
		justify-self: end;
		font-size: var(--text-xs);
		font-variant-numeric: tabular-nums;
		color: var(--muted);
	}
	.worse .delta {
		color: var(--bad);
	}
	.better .delta {
		color: var(--ok);
	}
</style>
