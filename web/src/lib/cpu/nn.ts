import { nnDigest } from '#lib/nn.ts';
import type { NnUnitParams } from '#lib/protocol/NnUnitParams.ts';
import type { NnUnitOutput } from '#lib/protocol/NnUnitOutput.ts';
import type { CpuPool } from './pool.ts';

/**
 * Trains a network unit on one of the pool's threads. Training cannot be split
 * across threads without changing the order of work, so a device trains several units
 * at once instead, one per thread (WorkerSession's lanes).
 */
export async function trainCpuNn(
	pool: CpuPool,
	p: NnUnitParams,
	signal?: AbortSignal
): Promise<NnUnitOutput> {
	const result = await pool.runOne({ template: 'nn', ...p }, signal);
	if (!('weights' in result)) throw new Error('the CPU thread returned no network');
	return { correct: result.correct, curve: result.curve, digest: await nnDigest(result.weights) };
}
