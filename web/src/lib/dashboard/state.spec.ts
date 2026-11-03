import { describe, expect, it } from 'vitest';
import {
	apply,
	CAUGHT_VISIBLE_MS,
	caughtDevices,
	deviceGroups,
	deviceLabel,
	emptyState,
	fastestDevicePerSecond,
	FEED_LENGTH,
	sortedJobs,
	speedUp,
	unitsPerSecond,
	THROUGHPUT_WINDOW_MS
} from './state.ts';
import type { JobSummary } from '#lib/protocol/JobSummary.ts';
import type { WorkerSummary } from '#lib/protocol/WorkerSummary.ts';

const job = (id: string, created_at: number, units_done = 0): JobSummary => ({
	id,
	network_id: 'n',
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
		s = apply(s, { type: 'worker_left', worker_id: 'w', units_returned: 3 }, 0);
		expect(s.workers.w).toBeUndefined();
		expect(s.feed[0]).toMatchObject({ kind: 'left', units_returned: 3 });
	});

	it('announces devices joining or leaving at once as one group', () => {
		let s = emptyState();
		for (const [i, t] of [0, 1000, 2500, 9000].entries()) {
			s = apply(s, { type: 'worker_joined', worker: worker(`w${i}`) }, t);
		}
		expect(s.feed.map((f) => (f.kind === 'joined' ? f.count : 0))).toEqual([1, 3]);

		for (const [i, t] of [20_000, 21_000, 22_000].entries()) {
			s = apply(s, { type: 'worker_left', worker_id: `w${i}`, units_returned: i }, t);
		}
		expect(s.feed[0]).toMatchObject({ kind: 'left', count: 3, units_returned: 3 });
	});

	it('keeps a caught cheater on show after it is gone, then drops it', () => {
		let s = apply(emptyState(), { type: 'worker_joined', worker: worker('c') }, 0);
		s = apply(s, { type: 'worker_banned', worker_id: 'c', wrong_results: 4, units_redone: 2 }, 100);
		expect(caughtDevices(s, 100).map((c) => [c.worker.id, c.left_at])).toEqual([['c', null]]);
		expect(s.feed[0]).toMatchObject({ kind: 'banned', wrong_results: 4, units_redone: 2 });

		s = apply(s, { type: 'worker_left', worker_id: 'c', units_returned: 1 }, 200);
		expect(s.workers.c).toBeUndefined();
		// The ban told the story; leaving adds nothing to the feed.
		expect(s.feed[0].kind).toBe('banned');
		expect(caughtDevices(s, 200 + CAUGHT_VISIBLE_MS - 1)).toHaveLength(1);
		expect(caughtDevices(s, 200 + CAUGHT_VISIBLE_MS)).toHaveLength(0);
	});

	it('adds up what disruptions cost and what verification caught', () => {
		let s = emptyState();
		for (const id of ['a', 'b', 'c']) {
			s = apply(s, { type: 'worker_joined', worker: worker(id) }, 0);
		}
		s = apply(s, { type: 'worker_left', worker_id: 'a', units_returned: 3 }, 10);
		s = apply(s, { type: 'worker_left', worker_id: 'b', units_returned: 2 }, 20);
		s = apply(s, { type: 'worker_banned', worker_id: 'c', wrong_results: 4, units_redone: 1 }, 30);
		// A caught device leaving is part of its ban, not a departure.
		s = apply(s, { type: 'worker_left', worker_id: 'c', units_returned: 1 }, 40);
		expect(s.totals).toEqual({
			banned: 1,
			wrongResults: 4,
			unitsRedone: 1,
			left: 2,
			unitsReturned: 5
		});
		// A reconnecting dashboard keeps its totals.
		s = apply(s, { type: 'snapshot', workers: [], jobs: [] }, 50);
		expect(s.totals.banned).toBe(1);
	});

	it('ignores a ban for a worker it never saw', () => {
		const s = apply(
			emptyState(),
			{ type: 'worker_banned', worker_id: 'x', wrong_results: 1, units_redone: 0 },
			0
		);
		expect(s.feed).toEqual([]);
	});

	it('keeps only the latest feed items', () => {
		let s = emptyState();
		for (let i = 0; i < FEED_LENGTH + 3; i++) {
			s = apply(s, { type: 'worker_joined', worker: worker(`w${i}`) }, i * 10_000);
		}
		expect(s.feed).toHaveLength(FEED_LENGTH);
		expect(new Set(s.feed.map((f) => f.key)).size).toBe(FEED_LENGTH);
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
				type: 'units_completed',
				units: 3,
				jobs: [{ job_id: 'j', units_done: 3, units_total: 10 }],
				workers: [
					{ worker_id: 'w', units: 2 },
					{ worker_id: 'gone', units: 1 }
				]
			},
			5
		);
		expect(after.jobs.j.units_done).toBe(3);
		expect(after.workers.w.units_done).toBe(2);
		expect(after.workers.gone).toBeUndefined();
		expect(before.jobs.j.units_done).toBe(0);
		expect(before.workers.w.units_done).toBe(0);
	});

	it('never moves job progress backwards on out-of-order events', () => {
		let s = apply(emptyState(), { type: 'snapshot', workers: [], jobs: [job('j', 1, 5)] }, 0);
		s = apply(
			s,
			{
				type: 'units_completed',
				units: 1,
				jobs: [{ job_id: 'j', units_done: 4, units_total: 10 }],
				workers: [{ worker_id: 'x', units: 1 }]
			},
			0
		);
		expect(s.jobs.j.units_done).toBe(5);
	});

	it('computes throughput over a sliding window', () => {
		let s = emptyState();
		for (const [t, units] of [
			[0, 1],
			[1000, 2],
			[2000, 3],
			[9000, 4]
		]) {
			s = apply(s, { type: 'units_completed', units, jobs: [], workers: [] }, t);
		}
		expect(unitsPerSecond(s, 9000)).toBeCloseTo(10 / (THROUGHPUT_WINDOW_MS / 1000));
		// At 11.5 s only the batches at 2 s and 9 s are inside the 10 s window.
		expect(unitsPerSecond(s, 11_500)).toBeCloseTo(7 / (THROUGHPUT_WINDOW_MS / 1000));
	});

	it('groups devices by kind, most numerous first', () => {
		const cpu = (id: string, units: number) => ({
			...worker(id),
			device: { backend: 'wasm' as const, vendor: 'cpu', architecture: '4 threads' },
			units_done: units
		});
		const s = apply(
			emptyState(),
			{ type: 'snapshot', workers: [worker('g'), cpu('a', 2), cpu('b', 3)], jobs: [] },
			0
		);
		expect(deviceGroups(s).map((g) => [g.vendor, g.count, g.units_done])).toEqual([
			['cpu', 2, 5],
			[worker('g').device.vendor, 1, 0]
		]);
	});

	it('compares all devices together with the fastest one alone', () => {
		const batch = (workers: [string, number][]) => ({
			type: 'units_completed' as const,
			units: workers.reduce((n, [, u]) => n + u, 0),
			jobs: [],
			workers: workers.map(([worker_id, units]) => ({ worker_id, units }))
		});
		let s = apply(emptyState(), batch([['fast', 6]]), 0);
		// One device alone: nothing to compare with.
		expect(speedUp(s, 0)).toBeNull();
		s = apply(
			s,
			batch([
				['fast', 4],
				['slow', 5],
				['slower', 5]
			]),
			1000
		);
		expect(fastestDevicePerSecond(s, 1000)).toBeCloseTo(10 / (THROUGHPUT_WINDOW_MS / 1000));
		// 20 units in all, the fastest did 10: twice as fast together.
		expect(speedUp(s, 1000)).toBeCloseTo(2);
	});

	it('names devices as people would', () => {
		expect(deviceLabel({ backend: 'webgpu', vendor: 'qualcomm', architecture: 'adreno' })).toBe(
			'видеокарта qualcomm'
		);
		expect(deviceLabel({ backend: 'wasm', vendor: 'cpu', architecture: '3 threads' })).toBe(
			'процесор (3 нишки)'
		);
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
