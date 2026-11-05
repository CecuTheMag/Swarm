/// <reference lib="webworker" />
// Runs kernels off the main thread. Only kernels bundled with the site can run,
// and only when their hash matches the one the coordinator pinned for the unit.
//
// Kernels run on the GPU (WebGPU) when there is one, otherwise on the CPU
// (WebAssembly). A template's pinned hash is that of its WGSL source; the CPU
// fallback runs the same template version, built from the Rust reference that the
// shared test vectors check against the WGSL kernel.

import { GpuUnavailableError, requestGpu } from '#lib/gpu/device.ts';
import { sha256Hex } from '#lib/gpu/hash.ts';
import { createPiKernel } from '#lib/gpu/pi.ts';
import { createPtKernel } from '#lib/gpu/pt.ts';
import { createNnKernel } from '#lib/gpu/nn.ts';
import { loadNnReference } from '#lib/nn.ts';
import { cpuThreads, createCpuPool } from '#lib/cpu/pool.ts';
import { runCpuPi } from '#lib/cpu/pi.ts';
import { renderCpuPt } from '#lib/cpu/pt.ts';
import { trainCpuNn } from '#lib/cpu/nn.ts';
import { encodeRgb } from '#lib/pt.ts';
import { fromBase64, toBase64 } from '#lib/bytes.ts';
import { ProgramRunner } from '#lib/program/runner.ts';
import { nnShader, piShader, ptShader } from '#lib/templates.ts';
import type { ComputeRequest, ComputeResponse } from './compute-messages.ts';
import type { Backend } from '#lib/protocol/Backend.ts';
import type { DeviceInfo } from '#lib/protocol/DeviceInfo.ts';
import type { JsonValue } from '#lib/protocol/serde_json/JsonValue.ts';
import type { PiUnitParams } from '#lib/protocol/PiUnitParams.ts';
import type { PtUnitParams } from '#lib/protocol/PtUnitParams.ts';
import type { NnUnitParams } from '#lib/protocol/NnUnitParams.ts';
import type { ProgramUnitParams } from '#lib/protocol/ProgramUnitParams.ts';
import type { TemplateRef } from '#lib/protocol/TemplateRef.ts';

interface Kernel {
	sha256: string;
	run(params: JsonValue, signal: AbortSignal): Promise<JsonValue>;
}

let kernels: Map<string, Kernel> | undefined;
// Units computing now; a CPU device trains several networks at once.
const current = new Set<AbortController>();

// A network's own programs (spec 0012), checked against their pinned hash and run
// without imports in a thread of their own.
const programs = new ProgramRunner();

const post = (msg: ComputeResponse) => self.postMessage(msg);

type Run = (params: JsonValue, signal: AbortSignal) => Promise<JsonValue>;
type Hashes = { pi: string; pt: string; nn: string };

/** Sets up the kernels on the GPU, or on the CPU if there is none or `backend` asks for it. */
async function init(backend?: Backend): Promise<DeviceInfo> {
	const hashes = {
		pi: await sha256Hex(piShader),
		pt: await sha256Hex(ptShader),
		nn: await sha256Hex(nnShader)
	};
	if (backend !== 'wasm') {
		try {
			return await initGpu(hashes);
		} catch (e) {
			if (!(e instanceof GpuUnavailableError)) throw e;
		}
	}
	return initCpu(hashes);
}

function register(hashes: Hashes, pi: Run, pt: Run, nn: Run) {
	kernels = new Map([
		['pi@1', { sha256: hashes.pi, run: pi }],
		['pt@1', { sha256: hashes.pt, run: pt }],
		['nn@1', { sha256: hashes.nn, run: nn }]
	]);
}

async function initGpu(hashes: Hashes): Promise<DeviceInfo> {
	const gpu = await requestGpu();
	const pi = createPiKernel(gpu.device);
	const pt = createPtKernel(gpu.device);
	// The network kernel needs the WebAssembly reference (dataset, initial weights);
	// loaded only once a network unit arrives.
	let nn: ReturnType<typeof createNnKernel> | undefined;
	register(
		hashes,
		async (params, signal) => ({ ...(await pi.run(params as PiUnitParams, signal)) }),
		async (params, signal) => ({ ...(await pt.run(params as PtUnitParams, signal)) }),
		async (params, signal) => {
			nn ??= createNnKernel(gpu.device, await loadNnReference());
			return { ...(await nn.run(params as NnUnitParams, signal)) };
		}
	);
	gpu.device.lost.then((info: GPUDeviceLostInfo) => {
		kernels = undefined;
		post({ kind: 'device_lost', reason: info.message });
	});
	return gpu.info;
}

function initCpu(hashes: Hashes): DeviceInfo {
	const pool = createCpuPool(cpuThreads());
	register(
		hashes,
		async (params, signal) => ({ ...(await runCpuPi(pool, params as PiUnitParams, signal)) }),
		async (params, signal) => ({
			rgb: encodeRgb(await renderCpuPt(pool, params as PtUnitParams, signal))
		}),
		async (params, signal) => ({ ...(await trainCpuNn(pool, params as NnUnitParams, signal)) })
	);
	return { backend: 'wasm', vendor: 'cpu', architecture: `${pool.threads} threads` };
}

/** What computes a unit of `template`, or why nothing may. */
function runner(template: TemplateRef): Run {
	if (template.id === 'program' && template.version === 1) {
		return async (params, signal) => {
			const { input } = params as ProgramUnitParams;
			const output = await programs.run(template.sha256, fromBase64(input), signal);
			return { output: toBase64(output) };
		};
	}
	const kernel = kernels?.get(`${template.id}@${template.version}`);
	if (!kernel) {
		throw new Error(`template ${template.id} v${template.version} is not available`);
	}
	if (kernel.sha256 !== template.sha256) {
		throw new Error(`template ${template.id} hash mismatch; refusing to run`);
	}
	return kernel.run;
}

self.onmessage = async (event: MessageEvent<ComputeRequest>) => {
	const req = event.data;
	let run: AbortController | undefined;
	if (req.kind === 'abort') {
		for (const controller of current) controller.abort();
		return;
	}
	try {
		if (req.kind === 'init') {
			post({ kind: 'reply', id: req.id, ok: true, value: await init(req.backend) });
			return;
		}
		const compute = runner(req.template);
		const controller = new AbortController();
		current.add(controller);
		run = controller;
		const started = performance.now();
		const output = await compute(req.params, controller.signal);
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
		if (run) current.delete(run);
	}
};
