// The Rust reference kernels in the console, to score a timetable a person edits exactly
// as the coordinator does (spec 0015). Loaded once, on first use.

import { kernelsWasmUrl } from '#lib/kernels-wasm.ts';
import type { TtScoreExports } from '#lib/tt.ts';

let kernels: Promise<TtScoreExports> | undefined;

export function scoreKernels(): Promise<TtScoreExports> {
	kernels ??= fetch(kernelsWasmUrl)
		.then((res) => {
			if (!res.ok) throw new Error(`the kernels did not load (${res.status})`);
			return res.arrayBuffer();
		})
		.then((bytes) => WebAssembly.instantiate(bytes, {}))
		.then(({ instance }) => instance.exports as unknown as TtScoreExports)
		.catch((e) => {
			// Tried again next time: a network hiccup is not forever.
			kernels = undefined;
			throw e;
		});
	return kernels;
}
