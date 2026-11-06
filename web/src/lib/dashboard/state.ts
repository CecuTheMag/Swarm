import type { DashboardEvent } from '#lib/protocol/DashboardEvent.ts';
import type { DeviceInfo } from '#lib/protocol/DeviceInfo.ts';
import type { JobSummary } from '#lib/protocol/JobSummary.ts';
import type { WorkerProgress } from '#lib/protocol/WorkerProgress.ts';
import type { WorkerSummary } from '#lib/protocol/WorkerSummary.ts';

/** A batch of decided units, as the coordinator announced it. */
export interface Batch {
	at: number;
	units: number;
	workers: WorkerProgress[];
}

/** A device caught returning wrong results. It stays on show for a while after its
 * connection closes, so the room sees what happened. */
export interface Caught {
	worker: WorkerSummary;
	at: number;
	wrong_results: number;
	units_redone: number;
	/** When its connection closed, if it has. */
	left_at: number | null;
}

/** What the projector's event list shows. */
export type FeedItem = { key: number; at: number } & (
	| { kind: 'joined'; count: number; device: DeviceInfo }
	| { kind: 'left'; count: number; device: DeviceInfo; units_returned: number }
	| { kind: 'banned'; device: DeviceInfo; wrong_results: number; units_redone: number }
);

/** What disruptions cost and what verification caught, since the page opened. */
export interface Totals {
	/** Devices blocked for wrong results. */
	banned: number;
	wrongResults: number;
	unitsRedone: number;
	/** Devices that left (switched off, cut off, closed the page), caught ones not counted. */
	left: number;
	/** Units those devices held, given back to the queue and computed elsewhere. */
	unitsReturned: number;
}

export interface DashboardState {
	workers: Record<string, WorkerSummary>;
	jobs: Record<string, JobSummary>;
	/** Batches of decided units within the throughput window. */
	recentUnits: Batch[];
	caught: Record<string, Caught>;
	/** Newest first. */
	feed: FeedItem[];
	/** Key of the next feed item. */
	seq: number;
	totals: Totals;
}

export const THROUGHPUT_WINDOW_MS = 10_000;
/** How long a caught device stays on show after it is gone. */
export const CAUGHT_VISIBLE_MS = 12_000;
export const FEED_LENGTH = 6;
/** Devices joining (or leaving) within this long of each other share one feed item. */
const GROUP_MS = 4_000;

export const emptyState = (): DashboardState => ({
	workers: {},
	jobs: {},
	recentUnits: [],
	caught: {},
	feed: [],
	seq: 0,
	totals: { banned: 0, wrongResults: 0, unitsRedone: 0, left: 0, unitsReturned: 0 }
});

const byId = <T extends { id: string }>(items: T[]) =>
	Object.fromEntries(items.map((item) => [item.id, item]));

type Unkeyed<T> = T extends unknown ? Omit<T, 'key'> : never;

/** The feed with `item` added on top. */
const withFeed = (state: DashboardState, item: Unkeyed<FeedItem>) => ({
	feed: [{ ...item, key: state.seq } as FeedItem, ...state.feed].slice(0, FEED_LENGTH),
	seq: state.seq + 1
});

/** Applies one coordinator event; returns a new state and never mutates the input. */
export function apply(state: DashboardState, event: DashboardEvent, now: number): DashboardState {
	const s = { ...state, caught: pruneCaught(state.caught, now) };
	switch (event.type) {
		case 'snapshot':
			return { ...s, workers: byId(event.workers), jobs: byId(event.jobs), recentUnits: [] };

		case 'worker_joined': {
			const workers = { ...s.workers, [event.worker.id]: event.worker };
			const last = s.feed[0];
			// A room joining at once is one line, not thirty.
			if (last?.kind === 'joined' && now - last.at < GROUP_MS) {
				const grouped: FeedItem = { ...last, at: now, count: last.count + 1 };
				return { ...s, workers, feed: [grouped, ...s.feed.slice(1)] };
			}
			return {
				...s,
				workers,
				...withFeed(s, { at: now, kind: 'joined', count: 1, device: event.worker.device })
			};
		}

		case 'worker_left': {
			const worker = s.workers[event.worker_id];
			const workers = { ...s.workers };
			delete workers[event.worker_id];
			const caught = s.caught[event.worker_id];
			if (caught) {
				// Its ban already told the story.
				return {
					...s,
					workers,
					caught: { ...s.caught, [event.worker_id]: { ...caught, left_at: now } }
				};
			}
			if (!worker) return { ...s, workers };
			const totals = {
				...s.totals,
				left: s.totals.left + 1,
				unitsReturned: s.totals.unitsReturned + event.units_returned
			};
			const last = s.feed[0];
			if (last?.kind === 'left' && now - last.at < GROUP_MS) {
				const grouped: FeedItem = {
					...last,
					at: now,
					count: last.count + 1,
					units_returned: last.units_returned + event.units_returned
				};
				return { ...s, workers, totals, feed: [grouped, ...s.feed.slice(1)] };
			}
			return {
				...s,
				workers,
				totals,
				...withFeed(s, {
					at: now,
					kind: 'left',
					count: 1,
					device: worker.device,
					units_returned: event.units_returned
				})
			};
		}

		case 'worker_banned': {
			// A cheater may be outvoted only after it left: it is still shown as caught.
			const worker = s.workers[event.worker.id] ?? event.worker;
			const { wrong_results, units_redone } = event;
			if (s.caught[worker.id]) return s;
			return {
				...s,
				totals: {
					...s.totals,
					banned: s.totals.banned + 1,
					wrongResults: s.totals.wrongResults + wrong_results,
					unitsRedone: s.totals.unitsRedone + units_redone
				},
				caught: {
					...s.caught,
					[worker.id]: {
						worker,
						at: now,
						wrong_results,
						units_redone,
						left_at: event.connected ? null : now
					}
				},
				...withFeed(s, {
					at: now,
					kind: 'banned',
					device: worker.device,
					wrong_results,
					units_redone
				})
			};
		}

		case 'job_updated':
			return { ...s, jobs: { ...s.jobs, [event.job.id]: event.job } };

		case 'units_completed': {
			// One batch per fraction of a second: copying the maps here is cheap, unlike
			// once per unit with a thousand devices.
			const workers = { ...s.workers };
			for (const { worker_id, units } of event.workers) {
				const worker = workers[worker_id];
				if (worker) workers[worker_id] = { ...worker, units_done: worker.units_done + units };
			}
			const jobs = { ...s.jobs };
			for (const { job_id, units_done } of event.jobs) {
				const job = jobs[job_id];
				// Batches can overtake each other; never go backwards.
				if (job) jobs[job_id] = { ...job, units_done: Math.max(job.units_done, units_done) };
			}
			return {
				...s,
				workers,
				jobs,
				recentUnits: [
					...prune(s.recentUnits, now),
					{ at: now, units: event.units, workers: event.workers }
				]
			};
		}
	}
}

const prune = (batches: Batch[], now: number) =>
	batches.filter(({ at }) => now - at < THROUGHPUT_WINDOW_MS);

function pruneCaught(caught: Record<string, Caught>, now: number) {
	const stale = Object.values(caught).filter(
		(c) => c.left_at !== null && now - c.left_at >= CAUGHT_VISIBLE_MS
	);
	if (stale.length === 0) return caught;
	const kept = { ...caught };
	for (const c of stale) delete kept[c.worker.id];
	return kept;
}

/** Completed units per second over the throughput window. */
export const unitsPerSecond = (state: DashboardState, now: number) =>
	prune(state.recentUnits, now).reduce((sum, { units }) => sum + units, 0) /
	(THROUGHPUT_WINDOW_MS / 1000);

/** Units per second of the single fastest device over the throughput window. */
export function fastestDevicePerSecond(state: DashboardState, now: number): number {
	const perWorker = new Map<string, number>();
	for (const { workers } of prune(state.recentUnits, now)) {
		for (const { worker_id, units } of workers) {
			perWorker.set(worker_id, (perWorker.get(worker_id) ?? 0) + units);
		}
	}
	return Math.max(0, ...perWorker.values()) / (THROUGHPUT_WINDOW_MS / 1000);
}

/** How many times faster all devices together are than the fastest one alone, or null
 * until at least two devices have contributed. */
export function speedUp(state: DashboardState, now: number): number | null {
	const contributors = new Set(
		prune(state.recentUnits, now).flatMap(({ workers }) => workers.map((w) => w.worker_id))
	);
	const fastest = fastestDevicePerSecond(state, now);
	if (contributors.size < 2 || fastest === 0) return null;
	return unitsPerSecond(state, now) / fastest;
}

/** Caught devices still on show. */
export const caughtDevices = (state: DashboardState, now: number) =>
	Object.values(pruneCaught(state.caught, now));

/** Jobs newest first. */
export const sortedJobs = (state: DashboardState) =>
	Object.values(state.jobs).sort((a, b) => b.created_at - a.created_at);

export interface DeviceGroup {
	key: string;
	vendor: string;
	architecture: string;
	backend: string;
	count: number;
	units_done: number;
}

/** Connected workers grouped by kind of device, most numerous first: a lab of identical
 * PCs is one row, not thirty. */
export function deviceGroups(state: DashboardState): DeviceGroup[] {
	const groups = new Map<string, DeviceGroup>();
	for (const { device, units_done } of Object.values(state.workers)) {
		const key = `${device.backend}|${device.vendor}|${device.architecture}`;
		const group = groups.get(key) ?? { key, ...device, count: 0, units_done: 0 };
		group.count += 1;
		group.units_done += units_done;
		groups.set(key, group);
	}
	return [...groups.values()].sort((a, b) => b.count - a.count || a.key.localeCompare(b.key));
}

/** A device as people would name it. */
export function deviceLabel(device: DeviceInfo): string {
	switch (device.backend) {
		case 'webgpu':
			return `видеокарта ${device.vendor}`;
		case 'wasm':
			return `процесор (${device.architecture.replace('threads', 'нишки')})`;
		case 'native':
			return 'сървър';
	}
}
