import type { Backend } from '#lib/protocol/Backend.ts';
import type { TemplateRef } from '#lib/protocol/TemplateRef.ts';
import type { JsonValue } from '#lib/protocol/serde_json/JsonValue.ts';

export type ComputeRequest =
	// `backend: 'wasm'` forces the CPU; otherwise the GPU is used when there is one.
	| { kind: 'init'; id: number; backend?: Backend }
	| { kind: 'run'; id: number; template: TemplateRef; params: JsonValue }
	| { kind: 'abort' };

export type ComputeResponse =
	| { kind: 'reply'; id: number; ok: true; value: unknown }
	| { kind: 'reply'; id: number; ok: false; error: string }
	| { kind: 'device_lost'; reason: string };
