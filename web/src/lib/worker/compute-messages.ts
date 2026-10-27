import type { TemplateRef } from '#lib/protocol/TemplateRef.ts';
import type { JsonValue } from '#lib/protocol/serde_json/JsonValue.ts';

export type ComputeRequest =
	| { kind: 'init'; id: number }
	| { kind: 'run'; id: number; template: TemplateRef; params: JsonValue }
	| { kind: 'abort' };

export type ComputeResponse =
	| { kind: 'reply'; id: number; ok: true; value: unknown }
	| { kind: 'reply'; id: number; ok: false; error: string }
	| { kind: 'device_lost'; reason: string };
