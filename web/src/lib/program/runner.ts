// Runs program units (spec 0008) for the compute worker: fetches each module once,
// checks it is exactly the one the coordinator named, and runs units in a separate
// thread with a time limit.

import { sha256OfBytes } from '#lib/bytes.ts';
import type { ProgramResult, ProgramTask } from './program.worker.ts';

/** A unit that takes longer is stopped and goes back to the queue for another device. */
export const UNIT_TIME_LIMIT_MS = 120_000;

export type FetchModule = (sha256: string) => Promise<Uint8Array>;

/** The module from the coordinator, by its hash. */
export const fetchModule: FetchModule = async (sha256) => {
	const res = await fetch(`/api/modules/${sha256}.wasm`);
	if (!res.ok) throw new Error(`could not download program ${sha256}: ${res.status}`);
	return new Uint8Array(await res.arrayBuffer());
};

const stopped = () => new DOMException('stopped', 'AbortError');

export class ProgramRunner {
	private modules = new Map<string, Promise<WebAssembly.Module>>();
	/** Threads that finished their last unit cleanly, kept for the next ones. */
	private idle: Worker[] = [];

	constructor(
		private fetch: FetchModule = fetchModule,
		private timeLimitMs = UNIT_TIME_LIMIT_MS,
		private spawn: () => Worker = () =>
			new Worker(new URL('./program.worker.ts', import.meta.url), { type: 'module' })
	) {}

	/** The compiled module with this hash; refuses bytes that hash differently. */
	module(sha256: string): Promise<WebAssembly.Module> {
		let module = this.modules.get(sha256);
		if (!module) {
			module = this.load(sha256);
			this.modules.set(sha256, module);
			// A failed download may succeed later.
			module.catch(() => this.modules.delete(sha256));
		}
		return module;
	}

	private async load(sha256: string): Promise<WebAssembly.Module> {
		const bytes = await this.fetch(sha256);
		if ((await sha256OfBytes(bytes)) !== sha256) {
			throw new Error(`program ${sha256} hash mismatch; refusing to run`);
		}
		const module = await WebAssembly.compile(bytes as BufferSource);
		// The coordinator refuses modules with imports; checked again here, so a device
		// never depends on the server for it.
		if (WebAssembly.Module.imports(module).length > 0) {
			throw new Error(`program ${sha256} has imports; refusing to run`);
		}
		return module;
	}

	/** Runs one unit of the program with this hash. */
	async run(sha256: string, input: Uint8Array, signal: AbortSignal): Promise<Uint8Array> {
		const module = await this.module(sha256);
		if (signal.aborted) throw stopped();
		const worker = this.idle.pop() ?? this.spawn();
		return new Promise((resolve, reject) => {
			const finish = (result: Uint8Array | Error, keep: boolean) => {
				clearTimeout(timer);
				signal.removeEventListener('abort', abort);
				worker.onmessage = worker.onerror = null;
				// Only a thread whose unit ended normally is reused; one that was stopped
				// mid-unit may still be running it.
				if (keep) this.idle.push(worker);
				else worker.terminate();
				if (result instanceof Error) reject(result);
				else resolve(result);
			};
			const timer = setTimeout(
				() =>
					finish(
						new Error(`the program ran longer than ${this.timeLimitMs / 1000} s; stopped`),
						false
					),
				this.timeLimitMs
			);
			const abort = () => finish(stopped(), false);
			signal.addEventListener('abort', abort, { once: true });
			worker.onmessage = (event: MessageEvent<ProgramResult>) => {
				const r = event.data;
				finish(r.ok ? r.output : new Error(r.error), true);
			};
			worker.onerror = (event) => {
				event.preventDefault();
				finish(new Error(event.message || 'the program thread failed'), false);
			};
			worker.postMessage({ module, input } satisfies ProgramTask);
		});
	}

	destroy() {
		for (const worker of this.idle) worker.terminate();
		this.idle = [];
	}
}
