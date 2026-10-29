import type { CpuResult, CpuTask } from './kernels.worker.ts';

/** Threads to compute on: all but one, so the device stays usable. */
export function cpuThreads(): number {
	return Math.max(1, (navigator.hardwareConcurrency || 2) - 1);
}

/** A fixed set of CPU threads (Web Workers), each running one task at a time. */
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

	return {
		threads,
		async runAll(next, done, signal) {
			const settled = await Promise.allSettled(
				workers.map(async (worker) => {
					for (let task = next(); task; task = next()) {
						signal?.throwIfAborted();
						// Awaited before `done` touches shared state: not `x += await …`,
						// which reads `x` first and loses updates from other threads.
						const result = await run(worker, task);
						done(task, result);
					}
				})
			);
			const failed = settled.find((r) => r.status === 'rejected');
			if (failed) throw failed.reason;
		},
		destroy() {
			for (const worker of workers) worker.terminate();
		}
	};
}
