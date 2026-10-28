/// <reference lib="webworker" />
// One CPU thread of the π kernel: counts a range of a unit's samples with the
// WebAssembly build of the Rust reference kernel.

import { kernelsWasmUrl } from '#lib/templates.ts';

export interface RangeRequest {
	seed: number;
	unit_index: number;
	start: number;
	end: number;
}

export type RangeResponse = { ok: true; inside: number } | { ok: false; error: string };

interface KernelExports {
	pi_count_inside_range(seed: number, unitIndex: number, start: number, end: number): number;
}

const kernels: Promise<KernelExports> = fetch(kernelsWasmUrl)
	.then((res) => res.arrayBuffer())
	.then((bytes) => WebAssembly.instantiate(bytes, {}))
	.then(({ instance }) => instance.exports as unknown as KernelExports);

const post = (msg: RangeResponse) => self.postMessage(msg);

self.onmessage = async (event: MessageEvent<RangeRequest>) => {
	const r = event.data;
	try {
		const k = await kernels;
		// WebAssembly returns the u32 count as a signed i32 with the same bits.
		post({ ok: true, inside: k.pi_count_inside_range(r.seed, r.unit_index, r.start, r.end) >>> 0 });
	} catch (e) {
		post({ ok: false, error: e instanceof Error ? e.message : String(e) });
	}
};
