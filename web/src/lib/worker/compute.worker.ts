/// <reference lib="webworker" />
// Runs kernels off the main thread. Only kernels bundled with the site can run,
// and only when their hash matches the one the coordinator pinned for the unit.

import { requestGpu } from '#lib/gpu/device.ts';
import { sha256Hex } from '#lib/gpu/hash.ts';
import { createPiKernel, shaderSource as piSource } from '#lib/gpu/pi.ts';
import type { ComputeRequest, ComputeResponse } from './compute-messages.ts';
import type { JsonValue } from '#lib/protocol/serde_json/JsonValue.ts';
import type { PiUnitParams } from '#lib/protocol/PiUnitParams.ts';

interface Kernel {
	sha256: string;
	run(params: JsonValue, signal: AbortSignal): Promise<JsonValue>;
}

let kernels: Map<string, Kernel> | undefined;
let current: AbortController | undefined;

const post = (msg: ComputeResponse) => self.postMessage(msg);

async function init() {
	const gpu = await requestGpu();
	const pi = createPiKernel(gpu.device);
	kernels = new Map([
		[
			'pi@1',
			{
				sha256: await sha256Hex(piSource),
				run: async (params, signal) => ({ ...(await pi.run(params as PiUnitParams, signal)) })
			}
		]
	]);
	gpu.device.lost.then((info: GPUDeviceLostInfo) => {
		kernels = undefined;
		post({ kind: 'device_lost', reason: info.message });
	});
	return gpu.info;
}

self.onmessage = async (event: MessageEvent<ComputeRequest>) => {
	const req = event.data;
	if (req.kind === 'abort') {
		current?.abort();
		return;
	}
	try {
		if (req.kind === 'init') {
			post({ kind: 'reply', id: req.id, ok: true, value: await init() });
			return;
		}
		const kernel = kernels?.get(`${req.template.id}@${req.template.version}`);
		if (!kernel) {
			throw new Error(`template ${req.template.id} v${req.template.version} is not available`);
		}
		if (kernel.sha256 !== req.template.sha256) {
			throw new Error(`template ${req.template.id} hash mismatch; refusing to run`);
		}
		current = new AbortController();
		const started = performance.now();
		const output = await kernel.run(req.params, current.signal);
		post({
			kind: 'reply',
			id: req.id,
			ok: true,
			value: { output, elapsed_ms: Math.round(performance.now() - started) }
		});
	} catch (e) {
		post({
			kind: 'reply',
			id: req.id,
			ok: false,
			error: e instanceof Error ? e.message : String(e)
		});
	} finally {
		current = undefined;
	}
};
