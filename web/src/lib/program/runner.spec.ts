import { readFileSync } from 'node:fs';
import { describe, expect, it, vi } from 'vitest';
import { sha256OfBytes } from '#lib/bytes.ts';
import { runUnit } from './abi.ts';
import { ProgramRunner } from './runner.ts';
import type { ProgramResult, ProgramTask } from './program.worker.ts';

const fixture = (name: string) =>
	new Uint8Array(readFileSync(new URL(`./fixtures/${name}.wasm`, import.meta.url)));

/**
 * A stand-in for the program thread: runs the unit in this process, or never answers
 * (`hang`), as an endless program would.
 */
class FakeWorker {
	onmessage: ((e: MessageEvent<ProgramResult>) => void) | null = null;
	onerror: ((e: ErrorEvent) => void) | null = null;
	terminated = false;
	constructor(private hang = false) {}
	postMessage(task: ProgramTask) {
		if (this.hang) return;
		runUnit(task.module, task.input).then(
			(output) => this.onmessage?.({ data: { ok: true, output } } as MessageEvent<ProgramResult>),
			(e: Error) =>
				this.onmessage?.({ data: { ok: false, error: e.message } } as MessageEvent<ProgramResult>)
		);
	}
	terminate() {
		this.terminated = true;
	}
}

function runner(modules: Record<string, Uint8Array>, workers: FakeWorker[], timeLimitMs = 1000) {
	const fetch = vi.fn(async (sha: string) => {
		const bytes = modules[sha];
		if (!bytes) throw new Error('404');
		return bytes;
	});
	let next = 0;
	const r = new ProgramRunner(fetch, timeLimitMs, () => workers[next++] as unknown as Worker);
	return { r, fetch };
}

describe('ProgramRunner', () => {
	it('runs units, downloading each module once and reusing its thread', async () => {
		const fnv = fixture('fnv');
		const sha = await sha256OfBytes(fnv);
		const worker = new FakeWorker();
		const { r, fetch } = runner({ [sha]: fnv }, [worker]);
		const signal = new AbortController().signal;
		const a = await r.run(sha, new Uint8Array([1]), signal);
		const b = await r.run(sha, new Uint8Array([1]), signal);
		expect(a).toEqual(b);
		expect(a.length).toBe(8);
		expect(fetch).toHaveBeenCalledTimes(1);
		expect(worker.terminated).toBe(false);
	});

	it('runs units side by side, one thread each', async () => {
		const fnv = fixture('fnv');
		const sha = await sha256OfBytes(fnv);
		const workers = [new FakeWorker(), new FakeWorker()];
		const { r } = runner({ [sha]: fnv }, workers);
		const signal = new AbortController().signal;
		const [a, b] = await Promise.all([
			r.run(sha, new Uint8Array([1]), signal),
			r.run(sha, new Uint8Array([2]), signal)
		]);
		expect(a).not.toEqual(b);
		// Both threads are kept; the next units reuse them.
		await Promise.all([r.run(sha, new Uint8Array(), signal), r.run(sha, new Uint8Array(), signal)]);
		expect(workers.some((w) => w.terminated)).toBe(false);
	});

	it('refuses a module whose hash differs from the one named', async () => {
		const fnv = fixture('fnv');
		const wrong = 'f'.repeat(64);
		const { r } = runner({ [wrong]: fnv }, [new FakeWorker()]);
		await expect(r.run(wrong, new Uint8Array(), new AbortController().signal)).rejects.toThrow(
			/hash mismatch/
		);
	});

	it('refuses a module with imports even when the hash matches', async () => {
		const imports = new Uint8Array([
			0, 97, 115, 109, 1, 0, 0, 0, 1, 4, 1, 96, 0, 0, 2, 9, 1, 3, 101, 110, 118, 1, 102, 0, 0
		]);
		const sha = await sha256OfBytes(imports);
		const { r } = runner({ [sha]: imports }, [new FakeWorker()]);
		await expect(r.run(sha, new Uint8Array(), new AbortController().signal)).rejects.toThrow(
			/imports/
		);
	});

	it('stops a unit that takes too long and starts the next on a fresh thread', async () => {
		const fnv = fixture('fnv');
		const sha = await sha256OfBytes(fnv);
		const stuck = new FakeWorker(true);
		const fresh = new FakeWorker();
		const { r } = runner({ [sha]: fnv }, [stuck, fresh], 50);
		const signal = new AbortController().signal;
		await expect(r.run(sha, new Uint8Array(), signal)).rejects.toThrow(/longer than/);
		expect(stuck.terminated).toBe(true);
		expect((await r.run(sha, new Uint8Array(), signal)).length).toBe(8);
	});

	it('stops when the contributor stops', async () => {
		const fnv = fixture('fnv');
		const sha = await sha256OfBytes(fnv);
		const stuck = new FakeWorker(true);
		const { r } = runner({ [sha]: fnv }, [stuck]);
		const stop = new AbortController();
		const running = r.run(sha, new Uint8Array(), stop.signal);
		// Once the unit is on its thread.
		await new Promise((resolve) => setTimeout(resolve, 20));
		stop.abort();
		await expect(running).rejects.toThrow(/stopped/);
		expect(stuck.terminated).toBe(true);
		// Stopped before it starts, nothing runs.
		await expect(r.run(sha, new Uint8Array(), stop.signal)).rejects.toThrow(/stopped/);
	});

	it('reports a failing unit and keeps the thread', async () => {
		const trap = fixture('trap');
		const sha = await sha256OfBytes(trap);
		const worker = new FakeWorker();
		const { r } = runner({ [sha]: trap }, [worker]);
		await expect(r.run(sha, new Uint8Array(), new AbortController().signal)).rejects.toThrow(
			/unreachable/
		);
		expect(worker.terminated).toBe(false);
	});

	it('retries a module whose download failed', async () => {
		const fnv = fixture('fnv');
		const sha = await sha256OfBytes(fnv);
		const modules: Record<string, Uint8Array> = {};
		const { r, fetch } = runner(modules, [new FakeWorker()]);
		await expect(r.run(sha, new Uint8Array(), new AbortController().signal)).rejects.toThrow('404');
		modules[sha] = fnv;
		expect((await r.run(sha, new Uint8Array(), new AbortController().signal)).length).toBe(8);
		expect(fetch).toHaveBeenCalledTimes(2);
	});
});
