/// <reference lib="webworker" />
// One CPU thread: runs a piece of a unit with the WebAssembly build of the Rust
// reference kernels.

import { kernelsWasmUrl } from '#lib/templates.ts';
import { loadNnReference, type NnReference } from '#lib/nn.ts';
import { solveTimetable, type TtExports } from '#lib/tt.ts';
import type { NnUnitParams } from '#lib/protocol/NnUnitParams.ts';

export type CpuTask =
	| { template: 'pi'; seed: number; unit_index: number; start: number; end: number }
	| {
			template: 'pt';
			width: number;
			height: number;
			x: number;
			y: number;
			w: number;
			h: number;
			spp: number;
			pass: number;
			seed: number;
			row_start: number;
			row_end: number;
	  }
	| ({ template: 'nn' } & NnUnitParams)
	| { template: 'tt'; school: Uint8Array; seed: number; index: number; iterations: number };

export type CpuResult =
	| { ok: true; inside: number }
	| { ok: true; rgb: Float32Array }
	| { ok: true; correct: number; curve: number[]; weights: Uint32Array }
	| { ok: true; slots: Uint8Array; cost: number }
	| { ok: false; error: string };

interface KernelExports extends TtExports {
	memory: WebAssembly.Memory;
	pi_count_inside_range(seed: number, unitIndex: number, start: number, end: number): number;
	pt_render_rows(...args: number[]): number;
}

const kernels: Promise<KernelExports> = fetch(kernelsWasmUrl)
	.then((res) => res.arrayBuffer())
	.then((bytes) => WebAssembly.instantiate(bytes, {}))
	.then(({ instance }) => instance.exports as unknown as KernelExports);

// The network's reference keeps its dataset between units; loaded on first use.
let nn: Promise<NnReference> | undefined;

self.onmessage = async (event: MessageEvent<CpuTask>) => {
	const t = event.data;
	try {
		if (t.template === 'nn') {
			nn ??= loadNnReference();
			const reference = await nn;
			await reference.prepare(t.dataset);
			const { correct, curve, weights } = reference.train(t);
			self.postMessage({ ok: true, correct, curve, weights } satisfies CpuResult, [weights.buffer]);
			return;
		}
		const k = await kernels;
		if (t.template === 'tt') {
			const { slots, cost } = solveTimetable(k, t.school, t.seed, t.index, t.iterations);
			self.postMessage({ ok: true, slots, cost } satisfies CpuResult, [slots.buffer]);
			return;
		}
		if (t.template === 'pi') {
			// WebAssembly returns the u32 count as a signed i32 with the same bits.
			const inside = k.pi_count_inside_range(t.seed, t.unit_index, t.start, t.end) >>> 0;
			self.postMessage({ ok: true, inside } satisfies CpuResult);
		} else {
			const ptr = k.pt_render_rows(
				t.width,
				t.height,
				t.x,
				t.y,
				t.w,
				t.h,
				t.spp,
				t.pass,
				t.seed,
				t.row_start,
				t.row_end
			);
			// Copy out of the module's memory, which the next call reuses.
			const rgb = new Float32Array(
				k.memory.buffer,
				ptr,
				(t.row_end - t.row_start) * t.w * 3
			).slice();
			self.postMessage({ ok: true, rgb } satisfies CpuResult, [rgb.buffer]);
		}
	} catch (e) {
		self.postMessage({
			ok: false,
			error: e instanceof Error ? e.message : String(e)
		} satisfies CpuResult);
	}
};
