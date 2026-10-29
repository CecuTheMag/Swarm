import { describe, expect, it } from 'vitest';
import { apply, emptyState, sortedJobs, unitsPerSecond, THROUGHPUT_WINDOW_MS } from './state.ts';
import type { JobSummary } from '#lib/protocol/JobSummary.ts';
import type { WorkerSummary } from '#lib/protocol/WorkerSummary.ts';

const job = (id: string, created_at: number, units_done = 0): JobSummary => ({
	id,
	template: 'pi',
	status: 'running',
	params: {},
	units_total: 10,
	units_done,
	error: null,
	result: null,
	created_at,
	finished_at: null
});

const worker = (id: string): WorkerSummary => ({
	id,
	device: { backend: 'webgpu', vendor: 'v', architecture: 'a' },
	connected_at: 0,
	units_done: 0
});

describe('dashboard state', () => {
	it('replaces everything on snapshot', () => {
		let s = apply(emptyState(), { type: 'worker_joined', worker: worker('old') }, 0);
		s = apply(s, { type: 'snapshot', workers: [worker('w')], jobs: [job('j', 1)] }, 0);
		expect(Object.keys(s.workers)).toEqual(['w']);
		expect(Object.keys(s.jobs)).toEqual(['j']);
	});

	it('tracks workers joining and leaving', () => {
		let s = apply(emptyState(), { type: 'worker_joined', worker: worker('w') }, 0);
		expect(s.workers.w).toBeDefined();
		s = apply(s, { type: 'worker_left', worker_id: 'w' }, 0);
		expect(s.workers.w).toBeUndefined();
	});

	it('counts unit completions per job and worker without mutating input', () => {
		const before = apply(
			emptyState(),
			{ type: 'snapshot', workers: [worker('w')], jobs: [job('j', 1)] },
			0
		);
		const after = apply(
			before,
			{
				type: 'unit_completed',
				job_id: 'j',
				unit_id: 'u',
				worker_id: 'w',
				units_done: 3,
				units_total: 10
			},
			5
		);
		expect(after.jobs.j.units_done).toBe(3);
		expect(after.workers.w.units_done).toBe(1);
		expect(before.jobs.j.units_done).toBe(0);
		expect(before.workers.w.units_done).toBe(0);
	});

	it('never moves job progress backwards on out-of-order events', () => {
		let s = apply(emptyState(), { type: 'snapshot', workers: [], jobs: [job('j', 1, 5)] }, 0);
		s = apply(
			s,
			{
				type: 'unit_completed',
				job_id: 'j',
				unit_id: 'u',
				worker_id: 'x',
				units_done: 4,
				units_total: 10
			},
			0
		);
		expect(s.jobs.j.units_done).toBe(5);
	});

	it('computes throughput over a sliding window', () => {
		let s = emptyState();
		for (const t of [0, 1000, 2000, 9000]) {
			s = apply(
				s,
				{
					type: 'unit_completed',
					job_id: 'j',
					unit_id: 'u',
					worker_id: 'w',
					units_done: 1,
					units_total: 1
				},
				t
			);
		}
		expect(unitsPerSecond(s, 9000)).toBeCloseTo(4 / (THROUGHPUT_WINDOW_MS / 1000));
		// At 11.5 s only the completions at 2 s and 9 s are inside the 10 s window.
		expect(unitsPerSecond(s, 11_500)).toBeCloseTo(2 / (THROUGHPUT_WINDOW_MS / 1000));
	});

	it('sorts jobs newest first', () => {
		const s = apply(
			emptyState(),
			{ type: 'snapshot', workers: [], jobs: [job('a', 1), job('b', 3), job('c', 2)] },
			0
		);
		expect(sortedJobs(s).map((j) => j.id)).toEqual(['b', 'c', 'a']);
	});
});
