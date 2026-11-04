import type { CpuResult, CpuTask } from './kernels.worker.ts';

/** Threads to compute on: all but one, so the device stays usable. */
export function cpuThreads(): number {
	return Math.max(1, (navigator.hardwareConcurrency || 2) - 1);
}

/**
 * A fixed set of CPU threads (Web Workers), each running one task at a time. Several
 * runs may share it: each task takes the next free thread.
 */
export interface CpuPool {
	readonly threads: number;
	/**
	 * Runs `next()`'s tasks on every thread until it returns `null`; `done` receives
	 * each result. Resolves only when no task is in flight, so the next run cannot
	 * receive a stale result.
	 */
	runAll(
		next: () => CpuTask | null,
		done: (task: CpuTask, result: CpuResult & { ok: true }) => void,
		signal?: AbortSignal
	): Promise<void>;
	/** Runs one task on the next free thread; other runs may share the pool meanwhile. */
	runOne(task: CpuTask, signal?: AbortSignal): Promise<CpuResult & { ok: true }>;
	destroy(): void;
}

export function createCpuPool(threads: number): CpuPool {
	const workers = Array.from(
		{ length: threads },
		() => new Worker(new URL('./kernels.worker.ts', import.meta.url), { type: 'module' })
	);

	function run(worker: Worker, task: CpuTask): Promise<CpuResult & { ok: true }> {
		return new Promise((resolve, reject) => {
			worker.onmessage = (event: MessageEvent<CpuResult>) => {
				if (event.data.ok) resolve(event.data);
				else reject(new Error(event.data.error));
			};
			worker.onerror = (event) => reject(new Error(event.message || 'CPU kernel worker failed'));
			worker.postMessage(task);
		});
	}

	// Threads not running a task; runs wait their turn for one.
	const free = [...workers];
	const waiting: ((worker: Worker) => void)[] = [];
	function acquire(): Promise<Worker> {
		const worker = free.pop();
		return worker ? Promise.resolve(worker) : new Promise((resolve) => waiting.push(resolve));
	}
	function release(worker: Worker) {
		const next = waiting.shift();
		if (next) next(worker);
		else free.push(worker);
	}
	async function runOne(task: CpuTask, signal?: AbortSignal) {
		signal?.throwIfAborted();
		const worker = await acquire();
		try {
			return await run(worker, task);
		} finally {
			release(worker);
		}
	}

	return {
		threads,
		async runAll(next, done, signal) {
			const settled = await Promise.allSettled(
				workers.map(async () => {
					for (let task = next(); task; task = next()) {
						// Awaited before `done` touches shared state: not `x += await …`,
						// which reads `x` first and loses updates from other threads.
						const result = await runOne(task, signal);
						done(task, result);
					}
				})
			);
			const failed = settled.find((r) => r.status === 'rejected');
			if (failed) throw failed.reason;
		},
		runOne,
		destroy() {
			for (const worker of workers) worker.terminate();
		}
	};
}
