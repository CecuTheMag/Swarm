import { sha256OfBytes } from '#lib/bytes.ts';
import { ttOutput } from '#lib/tt.ts';
import type { TtUnitOutput } from '#lib/protocol/TtUnitOutput.ts';
import type { TtUnitParams } from '#lib/protocol/TtUnitParams.ts';
import type { CpuPool } from './pool.ts';

// Schools by hash: a device downloads each once, however many searches it runs.
const schools = new Map<string, Promise<Uint8Array>>();

/** The school a unit names, downloaded once and checked against its hash. */
function school(sha256: string): Promise<Uint8Array> {
	let bytes = schools.get(sha256);
	if (!bytes) {
		bytes = (async () => {
			const res = await fetch(`/api/blobs/${sha256}.json`);
			if (!res.ok) throw new Error(`could not download the school: ${res.status}`);
			const data = new Uint8Array(await res.arrayBuffer());
			if ((await sha256OfBytes(data)) !== sha256) {
				throw new Error('school hash mismatch; refusing to run');
			}
			return data;
		})();
		schools.set(sha256, bytes);
		// A failed download may succeed later.
		bytes.catch(() => schools.delete(sha256));
	}
	return bytes;
}

/**
 * Runs one timetable search on one of the pool's threads (spec 0013). A search cannot
 * be split across threads without changing it, so a device runs several at once, one
 * per thread (WorkerSession's lanes).
 */
export async function solveCpuTt(
	pool: CpuPool,
	p: TtUnitParams,
	signal?: AbortSignal
): Promise<TtUnitOutput> {
	const data = await school(p.school);
	const result = await pool.runOne(
		// A copy per task: the thread takes ownership of what it is sent.
		{
			template: 'tt',
			school: data.slice(),
			seed: p.seed,
			index: p.index,
			iterations: p.iterations
		},
		signal
	);
	if (!('slots' in result)) throw new Error('the CPU thread returned no timetable');
	return ttOutput(result);
}
