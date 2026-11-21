import type { Backend } from '#lib/protocol/Backend.ts';
import type { DeviceInfo } from '#lib/protocol/DeviceInfo.ts';
import type { TemplateRef } from '#lib/protocol/TemplateRef.ts';
import type { JsonValue } from '#lib/protocol/serde_json/JsonValue.ts';
import type { ComputeRequest, ComputeResponse } from './compute-messages.ts';
import { UnfitError } from '#lib/ai/check.ts';

export interface UnitResult {
	output: JsonValue;
	elapsed_ms: number;
}

/** Distributes Omit over a union so each variant keeps its own fields. */
type WithoutId<T> = T extends unknown ? Omit<T, 'id'> : never;

type Pending = { resolve: (v: unknown) => void; reject: (e: Error) => void };

/** Promise-based handle on the compute Web Worker. */
export class ComputeClient {
	private worker = new Worker(new URL('./compute.worker.ts', import.meta.url), { type: 'module' });
	private nextId = 1;
	private pending = new Map<number, Pending>();

	constructor(onDeviceLost: (reason: string) => void) {
		this.worker.onmessage = (event: MessageEvent<ComputeResponse>) => {
			const msg = event.data;
			if (msg.kind === 'device_lost') {
				onDeviceLost(msg.reason);
				return;
			}
			const p = this.pending.get(msg.id);
			this.pending.delete(msg.id);
			if (msg.ok) p?.resolve(msg.value);
			else p?.reject(msg.unfit ? new UnfitError(msg.error) : new Error(msg.error));
		};
	}

	private call<T>(req: WithoutId<Extract<ComputeRequest, { id: number }>>): Promise<T> {
		const id = this.nextId++;
		return new Promise<T>((resolve, reject) => {
			this.pending.set(id, { resolve: resolve as (v: unknown) => void, reject });
			this.worker.postMessage({ ...req, id });
		});
	}

	init(backend?: Backend): Promise<DeviceInfo> {
		return this.call({ kind: 'init', backend });
	}

	run(template: TemplateRef, params: JsonValue): Promise<UnitResult> {
		return this.call({ kind: 'run', template, params });
	}

	abort() {
		this.worker.postMessage({ kind: 'abort' } satisfies ComputeRequest);
	}

	terminate() {
		this.worker.terminate();
		for (const p of this.pending.values()) p.reject(new Error('compute worker terminated'));
		this.pending.clear();
	}
}
