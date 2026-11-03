import { nnDigest } from '#lib/nn.ts';
import type { NnUnitParams } from '#lib/protocol/NnUnitParams.ts';
import type { NnUnitOutput } from '#lib/protocol/NnUnitOutput.ts';
import type { CpuPool } from './pool.ts';

/**
 * Trains a network unit on one of the pool's threads. Training cannot be split
 * across threads without changing the order of work, and every unit is capped at a
 * few seconds (kernels::nn::MAX_WORK), so one thread does it all.
 */
export async function trainCpuNn(
	pool: CpuPool,
	p: NnUnitParams,
	signal?: AbortSignal
): Promise<NnUnitOutput> {
	let given = false;
	let output: { correct: number; curve: number[]; weights: Uint32Array } | undefined;
	await pool.runAll(
		() => (given ? null : ((given = true), { template: 'nn', ...p })),
		(_, result) => {
			if ('weights' in result) output = result;
		},
		signal
	);
	if (!output) throw new Error('the CPU thread returned no network');
	return { correct: output.correct, curve: output.curve, digest: await nnDigest(output.weights) };
}
