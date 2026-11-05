// A network's own programs in the console (spec 0012): how a job's inputs are given,
// and how its outputs are saved.

import { fromHex, toBase64 } from '#lib/bytes.ts';
import { api } from '#lib/api.ts';
import type { JobOutputs } from '#lib/protocol/JobOutputs.ts';

/**
 * Unit inputs from a file: a JSON array of base64 strings, or else text with one input
 * per line (UTF-8). A final empty line is not a unit.
 */
export function parseInputs(text: string): string[] {
	const trimmed = text.trim();
	if (trimmed.startsWith('[')) {
		const parsed = JSON.parse(trimmed) as unknown;
		if (!Array.isArray(parsed) || !parsed.every((v) => typeof v === 'string')) {
			throw new Error('JSON файлът трябва да е масив от низове в base64.');
		}
		return parsed;
	}
	const lines = text.split(/\r?\n/);
	if (lines.at(-1) === '') lines.pop();
	const encoder = new TextEncoder();
	return lines.map((line) => toBase64(encoder.encode(line)));
}

/** Arguments for every unit, given as text or as hex. */
export function encodeArgs(text: string, asHex: boolean): string {
	if (!asHex) return toBase64(new TextEncoder().encode(text));
	try {
		return toBase64(fromHex(text));
	} catch {
		throw new Error('Аргументите не са валиден шестнадесетичен запис.');
	}
}

/** Saves a program job's outputs as a JSON file: one base64 string per unit, in order. */
export async function downloadOutputs(network: string, job: string) {
	const { outputs } = await api<JobOutputs>(`/api/networks/${network}/jobs/${job}/outputs`);
	const blob = new Blob([JSON.stringify({ job, outputs }, null, 1)], {
		type: 'application/json'
	});
	const a = document.createElement('a');
	a.href = URL.createObjectURL(blob);
	a.download = `swarm-${job}.json`;
	a.click();
	URL.revokeObjectURL(a.href);
}
