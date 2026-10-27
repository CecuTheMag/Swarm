import type { DashboardEvent } from '#lib/protocol/DashboardEvent.ts';
import type { JobSummary } from '#lib/protocol/JobSummary.ts';
import type { WorkerSummary } from '#lib/protocol/WorkerSummary.ts';

export interface DashboardState {
	workers: Record<string, WorkerSummary>;
	jobs: Record<string, JobSummary>;
	/** Completion timestamps (ms) within the throughput window. */
	recentUnits: number[];
}

export const THROUGHPUT_WINDOW_MS = 10_000;

export const emptyState = (): DashboardState => ({ workers: {}, jobs: {}, recentUnits: [] });

const byId = <T extends { id: string }>(items: T[]) =>
	Object.fromEntries(items.map((item) => [item.id, item]));

/** Applies one coordinator event; returns a new state and never mutates the input. */
export function apply(state: DashboardState, event: DashboardEvent, now: number): DashboardState {
	switch (event.type) {
		case 'snapshot':
			return { workers: byId(event.workers), jobs: byId(event.jobs), recentUnits: [] };

		case 'worker_joined':
			return { ...state, workers: { ...state.workers, [event.worker.id]: event.worker } };

		case 'worker_left': {
			const workers = { ...state.workers };
			delete workers[event.worker_id];
			return { ...state, workers };
		}

		case 'job_updated':
			return { ...state, jobs: { ...state.jobs, [event.job.id]: event.job } };

		case 'unit_completed': {
			const job = state.jobs[event.job_id];
			const worker = state.workers[event.worker_id];
			return {
				workers: worker
					? { ...state.workers, [worker.id]: { ...worker, units_done: worker.units_done + 1 } }
					: state.workers,
				jobs: job
					? {
							...state.jobs,
							// Events can arrive out of order across workers; never go backwards.
							[job.id]: { ...job, units_done: Math.max(job.units_done, event.units_done) }
						}
					: state.jobs,
				recentUnits: [...prune(state.recentUnits, now), now]
			};
		}
	}
}

const prune = (times: number[], now: number) => times.filter((t) => now - t < THROUGHPUT_WINDOW_MS);

/** Completed units per second over the throughput window. */
export const unitsPerSecond = (state: DashboardState, now: number) =>
	prune(state.recentUnits, now).length / (THROUGHPUT_WINDOW_MS / 1000);

/** Jobs newest first. */
export const sortedJobs = (state: DashboardState) =>
	Object.values(state.jobs).sort((a, b) => b.created_at - a.created_at);
