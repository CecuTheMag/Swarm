import { ptShader as shaderSource } from '#lib/templates.ts';
import { encodeRgb } from '#lib/pt.ts';
import type { PtUnitParams } from '#lib/protocol/PtUnitParams.ts';
import type { PtUnitOutput } from '#lib/protocol/PtUnitOutput.ts';

export { shaderSource };

const MAX_TILE = 128;
/** Paths per dispatch: small enough that slow GPUs finish well within driver watchdogs. */
const PATHS_PER_DISPATCH = 1 << 16;

export interface PtKernel {
	run(params: PtUnitParams, signal?: AbortSignal): Promise<PtUnitOutput>;
	/** The radiance sums themselves (for the self-test). */
	render(params: PtUnitParams, signal?: AbortSignal): Promise<Float32Array>;
	destroy(): void;
}

/** Compiles the path tracer once; `run` can then be called for many units. */
export function createPtKernel(device: GPUDevice): PtKernel {
	const module = device.createShaderModule({ code: shaderSource, label: 'pt' });
	const pipeline = device.createComputePipeline({
		layout: 'auto',
		compute: { module, entryPoint: 'main' },
		label: 'pt'
	});
	const maxBytes = MAX_TILE * MAX_TILE * 3 * 4;
	const paramsBuffer = device.createBuffer({
		size: 48,
		usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST
	});
	const sumsBuffer = device.createBuffer({
		size: maxBytes,
		usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC | GPUBufferUsage.COPY_DST
	});
	const readBuffer = device.createBuffer({
		size: maxBytes,
		usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST
	});
	const bindGroup = device.createBindGroup({
		layout: pipeline.getBindGroupLayout(0),
		entries: [
			{ binding: 0, resource: { buffer: paramsBuffer } },
			{ binding: 1, resource: { buffer: sumsBuffer } }
		]
	});

	async function render(p: PtUnitParams, signal?: AbortSignal): Promise<Float32Array> {
		if (p.w > MAX_TILE || p.h > MAX_TILE) throw new Error(`tile larger than ${MAX_TILE}`);
		const bytes = p.w * p.h * 3 * 4;
		device.queue.writeBuffer(sumsBuffer, 0, new Float32Array(p.w * p.h * 3));
		// Each dispatch adds the next samples to the running sums, one at a time in
		// order, exactly as the reference does.
		const chunk = Math.max(1, Math.floor(PATHS_PER_DISPATCH / (p.w * p.h)));
		for (let k = 0; k < p.spp; k += chunk) {
			signal?.throwIfAborted();
			const count = Math.min(chunk, p.spp - k);
			device.queue.writeBuffer(
				paramsBuffer,
				0,
				new Uint32Array([p.width, p.height, p.x, p.y, p.w, p.h, p.pass * p.spp + k, count, p.seed])
			);
			const encoder = device.createCommandEncoder();
			const pass = encoder.beginComputePass();
			pass.setPipeline(pipeline);
			pass.setBindGroup(0, bindGroup);
			pass.dispatchWorkgroups(Math.ceil(p.w / 8), Math.ceil(p.h / 8));
			pass.end();
			device.queue.submit([encoder.finish()]);
			await device.queue.onSubmittedWorkDone();
		}
		const encoder = device.createCommandEncoder();
		encoder.copyBufferToBuffer(sumsBuffer, 0, readBuffer, 0, bytes);
		device.queue.submit([encoder.finish()]);
		await readBuffer.mapAsync(GPUMapMode.READ, 0, bytes);
		const sums = new Float32Array(readBuffer.getMappedRange(0, bytes).slice(0));
		readBuffer.unmap();
		return sums;
	}

	return {
		render,
		run: async (p, signal) => ({ rgb: encodeRgb(await render(p, signal)) }),
		destroy() {
			paramsBuffer.destroy();
			sumsBuffer.destroy();
			readBuffer.destroy();
		}
	};
}
