import type { PiUnitParams } from '#lib/protocol/PiUnitParams.ts';
import type { PiUnitOutput } from '#lib/protocol/PiUnitOutput.ts';
import type { CpuPool } from './pool.ts';

/** Samples per task: tens of milliseconds on one thread, so stopping is responsive. */
export const CHUNK_SAMPLES = 1 << 22;

/**
 * Counts a π unit on the pool's threads. Each thread takes the next chunk of samples
 * until none are left; counts of disjoint ranges add up to the unit's count.
 */
export async function runCpuPi(
	pool: CpuPool,
	p: PiUnitParams,
	signal?: AbortSignal
): Promise<PiUnitOutput> {
	let next = 0;
	let inside = 0;
	await pool.runAll(
		() => {
			if (next >= p.samples) return null;
			const start = next;
			next = Math.min(start + CHUNK_SAMPLES, p.samples);
			return { template: 'pi', seed: p.seed, unit_index: p.unit_index, start, end: next };
		},
		(_, result) => {
			if ('inside' in result) inside += result.inside;
		},
		signal
	);
	return { inside };
}
