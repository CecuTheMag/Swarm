import { piShader as shaderSource } from '#lib/templates.ts';
import type { PiUnitParams } from '#lib/protocol/PiUnitParams.ts';
import type { PiUnitOutput } from '#lib/protocol/PiUnitOutput.ts';

export { shaderSource };

const WORKGROUP_SIZE = 256;
/** Samples per dispatch; small enough that slow GPUs finish well within driver watchdogs. */
export const CHUNK_SAMPLES = 1 << 22;
/** Each invocation handles about this many samples of a chunk. */
const SAMPLES_PER_INVOCATION = 64;

export interface PiKernel {
	run(params: PiUnitParams, signal?: AbortSignal): Promise<PiUnitOutput>;
	destroy(): void;
}

/** Compiles the pi kernel once; `run` can then be called for many units. */
export function createPiKernel(device: GPUDevice): PiKernel {
	const module = device.createShaderModule({ code: shaderSource, label: 'pi' });
	const pipeline = device.createComputePipeline({
		layout: 'auto',
		compute: { module, entryPoint: 'main' },
		label: 'pi'
	});
	const paramsBuffer = device.createBuffer({
		size: 16,
		usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST
	});
	const countBuffer = device.createBuffer({
		size: 4,
		usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC | GPUBufferUsage.COPY_DST
	});
	const readBuffer = device.createBuffer({
		size: 4,
		usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST
	});
	const bindGroup = device.createBindGroup({
		layout: pipeline.getBindGroupLayout(0),
		entries: [
			{ binding: 0, resource: { buffer: paramsBuffer } },
			{ binding: 1, resource: { buffer: countBuffer } }
		]
	});

	async function run(p: PiUnitParams, signal?: AbortSignal): Promise<PiUnitOutput> {
		device.queue.writeBuffer(countBuffer, 0, new Uint32Array([0]));
		for (let start = 0; start < p.samples; start += CHUNK_SAMPLES) {
			signal?.throwIfAborted();
			const end = Math.min(start + CHUNK_SAMPLES, p.samples);
			const invocations = Math.ceil((end - start) / SAMPLES_PER_INVOCATION);
			const workgroups = Math.max(1, Math.ceil(invocations / WORKGROUP_SIZE));

			// Queue operations execute in submission order, so each dispatch sees
			// the parameters written just before it.
			device.queue.writeBuffer(
				paramsBuffer,
				0,
				new Uint32Array([p.seed, p.unit_index, start, end])
			);
			const encoder = device.createCommandEncoder();
			const pass = encoder.beginComputePass();
			pass.setPipeline(pipeline);
			pass.setBindGroup(0, bindGroup);
			pass.dispatchWorkgroups(workgroups);
			pass.end();
			device.queue.submit([encoder.finish()]);
			// Keep at most one chunk in flight so stopping is responsive and the
			// GPU stays available to the rest of the system.
			await device.queue.onSubmittedWorkDone();
		}

		const encoder = device.createCommandEncoder();
		encoder.copyBufferToBuffer(countBuffer, 0, readBuffer, 0, 4);
		device.queue.submit([encoder.finish()]);
		await readBuffer.mapAsync(GPUMapMode.READ);
		const inside = new Uint32Array(readBuffer.getMappedRange().slice(0))[0];
		readBuffer.unmap();
		return { inside };
	}

	return {
		run,
		destroy() {
			paramsBuffer.destroy();
			countBuffer.destroy();
			readBuffer.destroy();
		}
	};
}
