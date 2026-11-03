import { nnShader as shaderSource } from '#lib/templates.ts';
import { nnDigest, nnSizes, type NnReference } from '#lib/nn.ts';
import type { NnUnitParams } from '#lib/protocol/NnUnitParams.ts';
import type { NnUnitOutput } from '#lib/protocol/NnUnitOutput.ts';

export { shaderSource };

const WORKGROUP_SIZE = 64;
/** Images per evaluation dispatch; also the largest batch. */
const SLOTS = 128;
/** Dynamic uniform offsets must be multiples of this (WebGPU's guaranteed minimum). */
const SLOT_BYTES = 256;
/** Training steps per submission: short enough that stopping is responsive. */
const STEPS_PER_SUBMIT = 20;
const NO_ACTS = 0xffffffff;
const CURVE_POINTS = 16;
/** `State` in nn.wgsl: step, correct, curve sums and counts, batch images and losses. */
const STATE_WORDS = 2 + 16 + 16 + 128 + 128;

export interface NnKernel {
	run(params: NnUnitParams, signal?: AbortSignal): Promise<NnUnitOutput>;
	destroy(): void;
}

interface Pass {
	fan_in: number;
	fan_out: number;
	w_off: number;
	in_off: number;
	out_off: number;
	delta_off: number;
	delta_in_off: number;
	hidden: number;
	samples: number;
	eval: number;
	chunk: number;
}

/**
 * Compiles the neural network kernel once; `run` then trains one configuration per
 * call, bit-identical to `kernels::nn::train`. The dataset and the initial weights
 * come from the WebAssembly reference, so every device starts from the same bytes.
 */
export function createNnKernel(device: GPUDevice, reference: NnReference): NnKernel {
	const module = device.createShaderModule({ code: shaderSource, label: 'nn' });
	const storage = (type: GPUBufferBindingType) => ({ buffer: { type } });
	const layout = device.createBindGroupLayout({
		entries: [
			{ binding: 0, visibility: GPUShaderStage.COMPUTE, buffer: { type: 'uniform' } },
			{
				binding: 1,
				visibility: GPUShaderStage.COMPUTE,
				buffer: { type: 'uniform', hasDynamicOffset: true }
			},
			{ binding: 2, visibility: GPUShaderStage.COMPUTE, ...storage('read-only-storage') },
			{ binding: 3, visibility: GPUShaderStage.COMPUTE, ...storage('read-only-storage') },
			{ binding: 4, visibility: GPUShaderStage.COMPUTE, ...storage('storage') },
			{ binding: 5, visibility: GPUShaderStage.COMPUTE, ...storage('storage') },
			{ binding: 6, visibility: GPUShaderStage.COMPUTE, ...storage('storage') },
			{ binding: 7, visibility: GPUShaderStage.COMPUTE, ...storage('storage') }
		]
	});
	const pipelineLayout = device.createPipelineLayout({ bindGroupLayouts: [layout] });
	const pipeline = (entryPoint: string) =>
		device.createComputePipeline({
			layout: pipelineLayout,
			compute: { module, entryPoint },
			label: `nn ${entryPoint}`
		});
	const pipelines = {
		pick: pipeline('pick'),
		forward: pipeline('forward'),
		softmax: pipeline('softmax_loss'),
		backward: pipeline('backward'),
		update: pipeline('update'),
		finish: pipeline('finish_step'),
		evaluate: pipeline('evaluate')
	};

	// The dataset is uploaded once per kernel and reused by every unit.
	const datasets = new Map<string, { pixels: GPUBuffer; labels: GPUBuffer }>();
	function dataset(name: string) {
		let buffers = datasets.get(name);
		if (!buffers) {
			const data = reference.dataset(name);
			const upload = (bytes: Uint8Array) => {
				const buffer = device.createBuffer({
					size: Math.ceil(bytes.length / 4) * 4,
					usage: GPUBufferUsage.STORAGE,
					mappedAtCreation: true
				});
				new Uint8Array(buffer.getMappedRange()).set(bytes);
				buffer.unmap();
				return buffer;
			};
			buffers = { pixels: upload(data.pixels), labels: upload(data.labels) };
			datasets.set(name, buffers);
		}
		return buffers;
	}

	async function run(p: NnUnitParams, signal?: AbortSignal): Promise<NnUnitOutput> {
		const data = reference.dataset(p.dataset);
		const { pixels, labels } = dataset(p.dataset);
		const sizes = nnSizes(p, data.dim);
		const layers = sizes.length - 1;
		const batch = 1 << p.batch_log2;

		// Offsets of each layer's weights, of its output activations and of its errors.
		const wOff: number[] = [];
		const actOff: number[] = [];
		let words = 0;
		let acts = 0;
		for (let l = 0; l < layers; l++) {
			wOff.push(words);
			words += sizes[l] * sizes[l + 1] + sizes[l + 1];
			actOff.push(acts);
			acts += SLOTS * sizes[l + 1];
		}

		const initial = reference.init(p);
		const created: GPUBuffer[] = [];
		const buffer = (size: number, usage: number) => {
			const b = device.createBuffer({ size: Math.max(16, size), usage });
			created.push(b);
			return b;
		};
		const weights = buffer(
			words * 4,
			GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC
		);
		device.queue.writeBuffer(weights, 0, initial);
		const actBuffer = buffer(acts * 4, GPUBufferUsage.STORAGE);
		const deltaBuffer = buffer(acts * 4, GPUBufferUsage.STORAGE);
		const state = buffer(
			STATE_WORDS * 4,
			GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC
		);
		device.queue.writeBuffer(state, 0, new Uint32Array(STATE_WORDS));

		const runUniform = buffer(32, GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST);
		const runWords = new Uint32Array([
			p.steps,
			batch,
			p.batch_log2,
			p.lr_q16,
			p.seed,
			data.trainLen,
			data.dim,
			layers
		]);
		device.queue.writeBuffer(runUniform, 0, runWords);

		// Every dispatch's parameters, one slot each, chosen with a dynamic offset.
		const passes: Pass[] = [];
		const slot = (pass: Pass) => passes.push(pass) - 1;
		const layerPass = (l: number, extra: Partial<Pass>): Pass => ({
			fan_in: sizes[l],
			fan_out: sizes[l + 1],
			w_off: wOff[l],
			in_off: l === 0 ? NO_ACTS : actOff[l - 1],
			out_off: actOff[l],
			delta_off: actOff[l],
			delta_in_off: l === 0 ? 0 : actOff[l - 1],
			hidden: l + 1 < layers ? 1 : 0,
			samples: batch,
			eval: 0,
			chunk: 0,
			...extra
		});
		const train = Array.from({ length: layers }, (_, l) => slot(layerPass(l, {})));
		const evalChunks: { forward: number[]; evaluate: number; samples: number }[] = [];
		for (let chunk = 0; chunk < data.valLen; chunk += SLOTS) {
			const samples = Math.min(SLOTS, data.valLen - chunk);
			const forward = Array.from({ length: layers }, (_, l) =>
				slot(layerPass(l, { samples, eval: 1, chunk }))
			);
			const evaluate = slot(layerPass(layers - 1, { samples, eval: 1, chunk }));
			evalChunks.push({ forward, evaluate, samples });
		}
		const slots = buffer(
			passes.length * SLOT_BYTES,
			GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST
		);
		const slotWords = new Uint32Array((passes.length * SLOT_BYTES) / 4);
		passes.forEach((pass, i) => {
			slotWords.set(
				[
					pass.fan_in,
					pass.fan_out,
					pass.w_off,
					pass.in_off,
					pass.out_off,
					pass.delta_off,
					pass.delta_in_off,
					pass.hidden,
					pass.samples,
					pass.eval,
					pass.chunk
				],
				(i * SLOT_BYTES) / 4
			);
		});
		device.queue.writeBuffer(slots, 0, slotWords);

		const bindGroup = device.createBindGroup({
			layout,
			entries: [
				{ binding: 0, resource: { buffer: runUniform } },
				{ binding: 1, resource: { buffer: slots, size: SLOT_BYTES } },
				{ binding: 2, resource: { buffer: pixels } },
				{ binding: 3, resource: { buffer: labels } },
				{ binding: 4, resource: { buffer: weights } },
				{ binding: 5, resource: { buffer: actBuffer } },
				{ binding: 6, resource: { buffer: deltaBuffer } },
				{ binding: 7, resource: { buffer: state } }
			]
		});

		const groups = (n: number) => Math.max(1, Math.ceil(n / WORKGROUP_SIZE));
		const dispatch = (
			cp: GPUComputePassEncoder,
			pipe: GPUComputePipeline,
			slotIndex: number,
			invocations: number
		) => {
			cp.setPipeline(pipe);
			cp.setBindGroup(0, bindGroup, [slotIndex * SLOT_BYTES]);
			cp.dispatchWorkgroups(groups(invocations));
		};

		// A rejected command buffer runs nothing and would return the initial weights:
		// fail the unit instead of reporting a wrong answer.
		device.pushErrorScope('validation');
		const failed = async () => {
			const error = await device.popErrorScope();
			if (error) throw new Error(`network kernel rejected: ${error.message}`);
		};
		try {
			for (let step = 0; step < p.steps; step += STEPS_PER_SUBMIT) {
				signal?.throwIfAborted();
				const encoder = device.createCommandEncoder();
				const cp = encoder.beginComputePass();
				for (let s = step; s < Math.min(step + STEPS_PER_SUBMIT, p.steps); s++) {
					dispatch(cp, pipelines.pick, train[0], batch);
					for (let l = 0; l < layers; l++) {
						dispatch(cp, pipelines.forward, train[l], batch * sizes[l + 1]);
					}
					dispatch(cp, pipelines.softmax, train[layers - 1], batch);
					for (let l = layers - 1; l > 0; l--) {
						dispatch(cp, pipelines.backward, train[l], batch * sizes[l]);
					}
					for (let l = 0; l < layers; l++) {
						dispatch(cp, pipelines.update, train[l], sizes[l] * sizes[l + 1] + sizes[l + 1]);
					}
					cp.setPipeline(pipelines.finish);
					cp.setBindGroup(0, bindGroup, [0]);
					cp.dispatchWorkgroups(1);
				}
				cp.end();
				device.queue.submit([encoder.finish()]);
				// One submission in flight: stopping stays responsive and the GPU
				// stays available to the rest of the system.
				await device.queue.onSubmittedWorkDone();
			}

			signal?.throwIfAborted();
			const encoder = device.createCommandEncoder();
			const cp = encoder.beginComputePass();
			for (const chunk of evalChunks) {
				for (let l = 0; l < layers; l++) {
					dispatch(cp, pipelines.forward, chunk.forward[l], chunk.samples * sizes[l + 1]);
				}
				dispatch(cp, pipelines.evaluate, chunk.evaluate, chunk.samples);
			}
			cp.end();
			const readWeights = buffer(words * 4, GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST);
			const readState = buffer(STATE_WORDS * 4, GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST);
			encoder.copyBufferToBuffer(weights, 0, readWeights, 0, words * 4);
			encoder.copyBufferToBuffer(state, 0, readState, 0, STATE_WORDS * 4);
			device.queue.submit([encoder.finish()]);
			await failed();
			await Promise.all([
				readWeights.mapAsync(GPUMapMode.READ),
				readState.mapAsync(GPUMapMode.READ)
			]);
			const finalWeights = new Uint32Array(readWeights.getMappedRange().slice(0));
			const s = new Uint32Array(readState.getMappedRange().slice(0));
			const curve = Array.from({ length: CURVE_POINTS }, (_, k) =>
				s[2 + 16 + k] === 0 ? 0 : Math.floor(s[2 + k] / s[2 + 16 + k])
			);
			return { correct: s[1], curve, digest: await nnDigest(finalWeights) };
		} finally {
			for (const b of created) b.destroy();
		}
	}

	return {
		run,
		destroy() {
			for (const { pixels, labels } of datasets.values()) {
				pixels.destroy();
				labels.destroy();
			}
		}
	};
}
