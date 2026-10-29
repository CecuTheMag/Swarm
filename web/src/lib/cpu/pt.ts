import type { PtUnitParams } from '#lib/protocol/PtUnitParams.ts';
import type { CpuPool } from './pool.ts';

/** Paths per task: tens of milliseconds on one thread, so stopping is responsive. */
const PATHS_PER_TASK = 1 << 14;

/**
 * Renders a path-tracer tile on the pool's threads, a few rows per task. Pixels are
 * independent, so the sums are bit-identical to the Rust reference however the rows
 * are shared out.
 */
export async function renderCpuPt(
	pool: CpuPool,
	p: PtUnitParams,
	signal?: AbortSignal
): Promise<Float32Array> {
	const sums = new Float32Array(p.w * p.h * 3);
	const rowsPerTask = Math.max(1, Math.floor(PATHS_PER_TASK / (p.w * p.spp)));
	let next = 0;
	await pool.runAll(
		() => {
			if (next >= p.h) return null;
			const row_start = next;
			next = Math.min(row_start + rowsPerTask, p.h);
			return { template: 'pt', ...p, row_start, row_end: next };
		},
		(task, result) => {
			if (task.template === 'pt' && 'rgb' in result) {
				sums.set(result.rgb, task.row_start * p.w * 3);
			}
		},
		signal
	);
	return sums;
}
