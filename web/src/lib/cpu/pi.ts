import type { PiUnitParams } from '#lib/protocol/PiUnitParams.ts';
import type { PiUnitOutput } from '#lib/protocol/PiUnitOutput.ts';
import type { RangeRequest, RangeResponse } from './pi.worker.ts';

/** Samples per task: tens of milliseconds on one thread, so stopping is responsive. */
export const CHUNK_SAMPLES = 1 << 22;

export interface CpuPiKernel {
	run(params: PiUnitParams, signal?: AbortSignal): Promise<PiUnitOutput>;
	destroy(): void;
}

/** Threads to compute on: all but one, so the device stays usable. */
export function cpuThreads(): number {
	return Math.max(1, (navigator.hardwareConcurrency || 2) - 1);
}

/**
 * Runs the π kernel on `threads` Web Workers. Each thread takes the next chunk of the
 * unit until none are left; counts of disjoint ranges add up to the unit's count.
 */
export function createCpuPiKernel(threads: number): CpuPiKernel {
	const workers = Array.from(
		{ length: threads },
		() => new Worker(new URL('./pi.worker.ts', import.meta.url), { type: 'module' })
	);

	function count(worker: Worker, req: RangeRequest): Promise<number> {
		return new Promise((resolve, reject) => {
			worker.onmessage = (event: MessageEvent<RangeResponse>) => {
				if (event.data.ok) resolve(event.data.inside);
				else reject(new Error(event.data.error));
			};
			worker.onerror = (event) => reject(new Error(event.message || 'CPU kernel worker failed'));
			worker.postMessage(req);
		});
	}

	async function run(p: PiUnitParams, signal?: AbortSignal): Promise<PiUnitOutput> {
		let next = 0;
		let inside = 0;
		const threadsDone = await Promise.allSettled(
			workers.map(async (worker) => {
				while (next < p.samples) {
					signal?.throwIfAborted();
					const start = next;
					const end = Math.min(start + CHUNK_SAMPLES, p.samples);
					next = end;
					// Not `inside += await …`: that reads `inside` before awaiting, so
					// threads finishing together would overwrite each other's counts.
					const chunkInside = await count(worker, {
						seed: p.seed,
						unit_index: p.unit_index,
						start,
						end
					});
					inside += chunkInside;
				}
			})
		);
		// allSettled waits until no thread has a chunk in flight, so the next run
		// cannot receive a stale count.
		const failed = threadsDone.find((r) => r.status === 'rejected');
		if (failed) throw failed.reason;
		return { inside };
	}

	return {
		run,
		destroy() {
			for (const worker of workers) worker.terminate();
		}
	};
}
