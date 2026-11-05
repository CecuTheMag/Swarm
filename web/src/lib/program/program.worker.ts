/// <reference lib="webworker" />
// Runs program units (spec 0008) in a thread of their own, so a program that never
// returns can be stopped by terminating this worker without touching anything else.

import { runUnit } from './abi.ts';

export interface ProgramTask {
	module: WebAssembly.Module;
	input: Uint8Array;
}

export type ProgramResult = { ok: true; output: Uint8Array } | { ok: false; error: string };

self.onmessage = async (event: MessageEvent<ProgramTask>) => {
	try {
		const output = await runUnit(event.data.module, event.data.input);
		self.postMessage({ ok: true, output } satisfies ProgramResult, [output.buffer]);
	} catch (e) {
		self.postMessage({
			ok: false,
			error: e instanceof Error ? e.message : String(e)
		} satisfies ProgramResult);
	}
};
