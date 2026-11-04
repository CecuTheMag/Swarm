import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { CoordinatorMessage } from '#lib/protocol/CoordinatorMessage.ts';
import type { WorkerMessage } from '#lib/protocol/WorkerMessage.ts';

// The session against a scripted coordinator and a compute engine whose units finish
// when the test says so: connection losses can be placed exactly where real networks
// put them only sometimes.

const runs: { lease: string; resolve: () => void }[] = [];

vi.mock('./compute-client.ts', () => ({
	ComputeClient: class {
		init = async () => ({ backend: 'wasm', vendor: 'cpu', architecture: 'test' });
		run = (_template: unknown, params: { lease: string }) =>
			new Promise((resolve) => {
				runs.push({
					lease: params.lease,
					resolve: () => resolve({ output: { inside: 1 }, elapsed_ms: 100 })
				});
			});
		abort() {}
		terminate() {}
	}
}));

class FakeSocket {
	static OPEN = 1;
	static all: FakeSocket[] = [];
	readyState = FakeSocket.OPEN;
	sent: WorkerMessage[] = [];
	onopen: (() => void) | null = null;
	onmessage: ((e: { data: string }) => void) | null = null;
	onclose: (() => void) | null = null;
	constructor(readonly url: string) {
		FakeSocket.all.push(this);
	}
	send(text: string) {
		this.sent.push(JSON.parse(text) as WorkerMessage);
	}
	close() {
		this.readyState = 3;
	}
	/** A message from the coordinator. */
	receive(msg: CoordinatorMessage) {
		this.onmessage?.({ data: JSON.stringify(msg) });
	}
	/** The network drops the connection. */
	drop() {
		this.readyState = 3;
		this.onclose?.();
	}
	requests() {
		return this.sent.filter((m) => m.type === 'request_work').length;
	}
}

const { WorkerSession } = await import('./session.svelte.ts');

const assign = (lease: string): CoordinatorMessage => ({
	type: 'assign',
	lease_id: lease,
	unit_id: lease,
	template: { id: 'pi', version: 1, sha256: '' },
	params: { lease },
	lease_expires_at: 0
});

/** Connects the latest socket as the coordinator would. */
function welcome(ws: FakeSocket) {
	ws.onopen?.();
	ws.receive({ type: 'welcome', worker_id: 'w', heartbeat_interval_ms: 10_000 });
}

beforeEach(() => {
	vi.useFakeTimers();
	vi.stubGlobal('WebSocket', FakeSocket);
	runs.length = 0;
	FakeSocket.all = [];
});

afterEach(() => {
	vi.useRealTimers();
	vi.unstubAllGlobals();
});

describe('WorkerSession', () => {
	it('computes the units of a new connection although the old unit was still running', async () => {
		const session = new WorkerSession('ws://test/ws/worker');
		await session.start();
		const first = FakeSocket.all[0];
		welcome(first);
		expect(first.requests()).toBe(2);
		first.receive(assign('a'));
		first.receive(assign('b'));
		await vi.waitFor(() => expect(runs.map((r) => r.lease)).toEqual(['a']));

		// Signal lost while unit a computes; the page reconnects after its back-off and
		// gets new units before a is done.
		first.drop();
		await vi.advanceTimersByTimeAsync(1000);
		const second = FakeSocket.all[1];
		expect(second).toBeDefined();
		welcome(second);
		second.receive(assign('c'));
		second.receive(assign('d'));

		// a finishes: its result belongs to the lost connection and is dropped; c is next.
		runs[0].resolve();
		await vi.waitFor(() => expect(runs.map((r) => r.lease)).toEqual(['a', 'c']));
		expect(second.sent.some((m) => m.type === 'result' && m.lease_id === 'a')).toBe(false);

		runs[1].resolve();
		await vi.waitFor(() => expect(runs.map((r) => r.lease)).toEqual(['a', 'c', 'd']));
		expect(second.sent.filter((m) => m.type === 'result').map((m) => m.lease_id)).toEqual(['c']);
		session.destroy();
	});

	it('reconnects with growing back-off while the coordinator is unreachable', async () => {
		const session = new WorkerSession('ws://test/ws/worker');
		await session.start();
		FakeSocket.all[0].drop();
		expect(session.status).toBe('reconnecting');
		for (const delay of [1000, 2000, 4000]) {
			const before = FakeSocket.all.length;
			await vi.advanceTimersByTimeAsync(delay - 1);
			expect(FakeSocket.all.length).toBe(before);
			await vi.advanceTimersByTimeAsync(1);
			expect(FakeSocket.all.length).toBe(before + 1);
			FakeSocket.all.at(-1)!.drop();
		}
		// Once connected again, the back-off starts over.
		await vi.advanceTimersByTimeAsync(8000);
		welcome(FakeSocket.all.at(-1)!);
		FakeSocket.all.at(-1)!.drop();
		const before = FakeSocket.all.length;
		await vi.advanceTimersByTimeAsync(1000);
		expect(FakeSocket.all.length).toBe(before + 1);
		session.destroy();
	});

	it('treats a connection that stops answering as lost', async () => {
		const session = new WorkerSession('ws://test/ws/worker');
		await session.start();
		const first = FakeSocket.all[0];
		welcome(first);
		// Two work requests go unanswered: a half-open connection, as after a phone
		// switches networks.
		await vi.advanceTimersByTimeAsync(20_000);
		expect(session.status).toBe('reconnecting');
		await vi.advanceTimersByTimeAsync(1000);
		expect(FakeSocket.all.length).toBe(2);
		session.destroy();
	});

	it('pausing finishes the unit in hand, then disconnects without reconnecting', async () => {
		const session = new WorkerSession('ws://test/ws/worker');
		session.power = 100;
		await session.start();
		const ws = FakeSocket.all[0];
		welcome(ws);
		ws.receive(assign('a'));
		ws.receive(assign('b'));
		await vi.waitFor(() => expect(runs.map((r) => r.lease)).toEqual(['a']));

		session.setPause('hidden');
		runs[0].resolve();
		await vi.waitFor(() =>
			expect(ws.sent.filter((m) => m.type === 'result').map((m) => m.lease_id)).toEqual(['a'])
		);
		// b, fetched in advance, is not started: it goes back to the queue on disconnect.
		expect(runs.map((r) => r.lease)).toEqual(['a']);
		expect(ws.readyState).toBe(FakeSocket.OPEN);
		ws.receive({ type: 'ack', lease_id: 'a', accepted: true });
		expect(ws.readyState).toBe(3);
		expect(session.status).toBe('paused');
		expect(session.unitsAccepted).toBe(1);
		await vi.advanceTimersByTimeAsync(60_000);
		expect(FakeSocket.all.length).toBe(1);

		// Back to the page: a new connection.
		session.setPause(null);
		expect(FakeSocket.all.length).toBe(2);
		welcome(FakeSocket.all[1]);
		expect(FakeSocket.all[1].requests()).toBeGreaterThan(0);
		session.destroy();
	});

	it('pausing while idle disconnects once owed answers arrive, or after a timeout', async () => {
		const session = new WorkerSession('ws://test/ws/worker');
		await session.start();
		const ws = FakeSocket.all[0];
		welcome(ws);
		session.setPause('battery');
		// Two work requests are still unanswered.
		expect(ws.readyState).toBe(FakeSocket.OPEN);
		await vi.advanceTimersByTimeAsync(3000);
		expect(ws.readyState).toBe(3);
		expect(session.status).toBe('paused');
		session.destroy();
	});

	it('a device that resumes before it disconnected keeps its connection', async () => {
		const session = new WorkerSession('ws://test/ws/worker');
		await session.start();
		const ws = FakeSocket.all[0];
		welcome(ws);
		session.setPause('hidden');
		session.setPause(null);
		ws.receive(assign('a'));
		await vi.waitFor(() => expect(runs.map((r) => r.lease)).toEqual(['a']));
		await vi.advanceTimersByTimeAsync(5000);
		expect(ws.readyState).toBe(FakeSocket.OPEN);
		expect(FakeSocket.all.length).toBe(1);
		session.destroy();
	});

	it('a CPU device trains one network per thread at once, other units alone', async () => {
		vi.stubGlobal('navigator', { hardwareConcurrency: 4 });
		const session = new WorkerSession('ws://test/ws/worker');
		await session.start();
		const ws = FakeSocket.all[0];
		welcome(ws);
		const nn = (lease: string): CoordinatorMessage => ({
			...(assign(lease) as Extract<CoordinatorMessage, { type: 'assign' }>),
			template: { id: 'nn', version: 1, sha256: '' }
		});
		ws.receive(nn('a'));
		// Three lanes (all threads but one) and one unit in advance: four held.
		expect(ws.requests()).toBe(4);
		ws.receive(nn('b'));
		ws.receive(nn('c'));
		ws.receive(nn('d'));
		ws.receive(assign('pi'));
		await vi.waitFor(() => expect(runs.map((r) => r.lease)).toEqual(['a', 'b', 'c']));

		// A thread frees up: the next network starts; the π unit waits for all of them.
		runs[1].resolve();
		await vi.waitFor(() => expect(runs.map((r) => r.lease)).toEqual(['a', 'b', 'c', 'd']));
		runs[0].resolve();
		runs[2].resolve();
		await vi.advanceTimersByTimeAsync(0);
		expect(runs).toHaveLength(4);
		runs[3].resolve();
		await vi.waitFor(() => expect(runs.map((r) => r.lease).at(-1)).toBe('pi'));
		expect(ws.sent.filter((m) => m.type === 'result').map((m) => m.lease_id)).toEqual([
			'b',
			'a',
			'c',
			'd'
		]);
		session.destroy();
	});

	it('rests between units at reduced power', async () => {
		const session = new WorkerSession('ws://test/ws/worker');
		session.power = 50;
		await session.start();
		const ws = FakeSocket.all[0];
		welcome(ws);
		ws.receive(assign('a'));
		ws.receive(assign('b'));
		await vi.waitFor(() => expect(runs.map((r) => r.lease)).toEqual(['a']));
		// a took 100 ms; at 50 % the device rests 100 ms before b.
		runs[0].resolve();
		await vi.advanceTimersByTimeAsync(99);
		expect(runs.map((r) => r.lease)).toEqual(['a']);
		await vi.advanceTimersByTimeAsync(1);
		expect(runs.map((r) => r.lease)).toEqual(['a', 'b']);
		session.destroy();
	});

	it('a pause cuts a rest short', async () => {
		const session = new WorkerSession('ws://test/ws/worker');
		session.power = 25;
		await session.start();
		const ws = FakeSocket.all[0];
		welcome(ws);
		ws.receive(assign('a'));
		await vi.waitFor(() => expect(runs.length).toBe(1));
		runs[0].resolve();
		await vi.advanceTimersByTimeAsync(10);
		ws.receive({ type: 'ack', lease_id: 'a', accepted: true });
		ws.receive({ type: 'no_work', retry_after_ms: 5000 });
		ws.receive({ type: 'no_work', retry_after_ms: 5000 });
		// Resting 300 ms after a; the pause does not wait for it.
		session.setPause('hidden');
		await vi.advanceTimersByTimeAsync(0);
		expect(ws.readyState).toBe(3);
		session.destroy();
	});

	it('joins the network of its link and obeys its owner (spec 0010)', async () => {
		const session = new WorkerSession('ws://test/ws/worker', undefined, 'lab12code');
		await session.start();
		const ws = FakeSocket.all[0];
		welcome(ws);
		expect(ws.sent[0]).toMatchObject({ type: 'hello', join_code: 'lab12code' });
		const asked = ws.requests();
		expect(asked).toBeGreaterThan(0);

		// Paused by the owner: no more requests, and a control message is no answer to
		// one (owed replies still time out as before).
		ws.receive({ type: 'control', control: { state: 'paused_by_owner' }, max_power: 100 });
		for (let i = 0; i < asked; i++) ws.receive({ type: 'no_work', retry_after_ms: 100 });
		await vi.advanceTimersByTimeAsync(5_000);
		expect(ws.requests()).toBe(asked);
		expect(session.status).toBe('held');
		expect(ws.readyState).toBe(FakeSocket.OPEN);

		// Running again: it asks for work at once, within the network's power limit.
		ws.receive({ type: 'control', control: { state: 'running' }, max_power: 50 });
		expect(ws.requests()).toBeGreaterThan(asked);
		session.power = 100;
		expect(session.effectivePower).toBe(50);
		session.destroy();
	});

	it('a removed device stops instead of reconnecting', async () => {
		const session = new WorkerSession('ws://test/ws/worker', undefined, 'lab12code');
		await session.start();
		const ws = FakeSocket.all[0];
		welcome(ws);
		ws.receive({ type: 'control', control: { state: 'removed' }, max_power: 100 });
		ws.receive({ type: 'error', message: 'this device was removed from the network' });
		ws.drop();
		await vi.advanceTimersByTimeAsync(30_000);
		expect(session.status).toBe('stopped');
		expect(FakeSocket.all.length).toBe(1);
		session.destroy();
	});
});
