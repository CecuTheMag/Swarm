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
});
