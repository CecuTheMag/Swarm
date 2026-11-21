// Runs language-model units (spec 0017) in the compute worker: a whole small model from
// the catalog, on the GPU (WebGPU) or the CPU (WebAssembly), decoding greedily so two
// honest devices compute the same tokens. transformers.js and its runtime load only when
// the first such unit arrives.

import ortWasmUrl from 'onnxruntime-web/ort-wasm-simd-threaded.asyncify.wasm?url';
import ortMjsUrl from 'onnxruntime-web/ort-wasm-simd-threaded.asyncify.mjs?url';
import { MODELS_PATH, verifiedFetch } from './files.ts';
import type { AiCatalog } from '#lib/protocol/AiCatalog.ts';
import type { AiModel } from '#lib/protocol/AiModel.ts';
import type { AiUnitOutput } from '#lib/protocol/AiUnitOutput.ts';
import type { AiUnitParams } from '#lib/protocol/AiUnitParams.ts';

type Transformers = typeof import('@huggingface/transformers');
type Loaded = {
	tokenizer: Awaited<ReturnType<Transformers['AutoTokenizer']['from_pretrained']>>;
	model: Awaited<ReturnType<Transformers['AutoModelForCausalLM']['from_pretrained']>>;
};

export interface AiRunner {
	run(params: AiUnitParams, signal: AbortSignal): Promise<AiUnitOutput>;
}

/** What `onFile` reports: where each model file came from, for the measurements. */
export type FileSource = (path: string, from: 'cache' | 'network', bytes: number) => void;

export function createAiRunner(
	catalog: AiCatalog,
	device: 'webgpu' | 'wasm',
	onFile?: FileSource
): AiRunner {
	let transformers: Promise<Transformers> | undefined;
	const models = new Map<string, Promise<Loaded>>();

	const runtime = () =>
		(transformers ??= import('@huggingface/transformers').then((t) => {
			const { env } = t;
			// Only this site's catalog files, checked by their hashes; never Hugging Face.
			env.allowRemoteModels = false;
			env.allowLocalModels = true;
			env.localModelPath = MODELS_PATH;
			env.useBrowserCache = false;
			// The runtime's own WebAssembly comes from this site too, loaded as a module
			// rather than a blob URL, which the Content-Security-Policy would refuse.
			env.useWasmCache = false;
			const onnx = env.backends.onnx as { wasm?: { wasmPaths?: unknown } };
			if (onnx.wasm) onnx.wasm.wasmPaths = { wasm: ortWasmUrl, mjs: ortMjsUrl };
			env.fetch = verifiedFetch(catalog, {
				fetch: (...args) => fetch(...args),
				caches: 'caches' in self ? self.caches : undefined,
				origin: self.location.origin,
				onFile
			});
			return t;
		}));

	const load = (model: AiModel): Promise<Loaded> => {
		let loaded = models.get(model.id);
		if (!loaded) {
			loaded = (async () => {
				const t = await runtime();
				const tokenizer = await t.AutoTokenizer.from_pretrained(model.id);
				const lm = await t.AutoModelForCausalLM.from_pretrained(model.id, {
					dtype: model.dtype as 'fp32',
					device
				});
				return { tokenizer, model: lm };
			})();
			// A failed load (a refused file, a lost GPU) is tried again with the next unit.
			loaded.catch(() => models.delete(model.id));
			models.set(model.id, loaded);
		}
		return loaded;
	};

	return {
		async run(p, signal) {
			const model = catalog.models.find((m) => m.id === p.model);
			if (!model) throw new Error(`model ${p.model} is not in the catalog`);
			signal.throwIfAborted();
			const t = await runtime();
			const { tokenizer, model: lm } = await load(model);
			signal.throwIfAborted();

			const inputs = model.chat
				? (tokenizer.apply_chat_template(
						[
							...(p.system ? [{ role: 'system', content: p.system }] : []),
							{ role: 'user', content: p.prompt }
						],
						{ add_generation_prompt: true, return_dict: true }
					) as Record<string, unknown>)
				: (tokenizer(p.system ? `${p.system}\n\n${p.prompt}` : p.prompt) as Record<
						string,
						unknown
					>);
			const promptLength = (inputs.input_ids as { dims: number[] }).dims.at(-1)!;

			// For each generated token: the chosen logit minus the runner-up's.
			const margins: number[] = [];
			// The list passes each processor's return value on, so it returns the logits
			// unchanged (the bundled types say `void`).
			class Margins extends t.LogitsProcessor {
				_call(_ids: bigint[][], logits: InstanceType<Transformers['Tensor']>) {
					const scores = (logits as unknown as { data: Float32Array }[])[0].data;
					let best = -Infinity;
					let second = -Infinity;
					for (let i = 0; i < scores.length; i++) {
						const s = scores[i];
						if (s > best) {
							second = best;
							best = s;
						} else if (s > second) {
							second = s;
						}
					}
					margins.push(Number.isFinite(second) ? best - second : 0);
					return logits;
				}
			}
			// „Спри“ stops the generation after the token being computed.
			class Stop extends t.StoppingCriteria {
				_call(ids: number[][]) {
					return ids.map(() => signal.aborted);
				}
			}
			const processors = new t.LogitsProcessorList();
			processors.push(new Margins());
			const criteria = new t.StoppingCriteriaList();
			criteria.push(new Stop());

			const sequences = (await lm.generate({
				...inputs,
				max_new_tokens: p.max_new_tokens,
				do_sample: false,
				num_beams: 1,
				repetition_penalty: 1,
				no_repeat_ngram_size: 0,
				logits_processor: processors,
				stopping_criteria: criteria
			} as unknown as Parameters<typeof lm.generate>[0])) as { tolist(): bigint[][] };
			signal.throwIfAborted();

			const tokens = sequences.tolist()[0].slice(promptLength).map(Number);
			const text = tokenizer.decode(tokens, { skip_special_tokens: true });
			return { text, tokens, margins: margins.slice(0, tokens.length) };
		}
	};
}
