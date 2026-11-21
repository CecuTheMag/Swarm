// The known-answer check a device runs whenever it loads a language model (spec 0017):
// a GPU or driver that computes the model wrongly gives a wrong answer from the first
// tokens on, and such a device takes none of the job's units.

import type { AiModelCheck } from '#lib/protocol/AiModelCheck.ts';

/**
 * Two answers that part at a token both devices chose by less than this are the same
 * answer: an honest near-tie. The same as `protocol::templates::ai::NEAR_TIE`.
 */
export const NEAR_TIE = 0.5;

export interface Answer {
	tokens: number[];
	margins: number[];
}

/**
 * Whether two answers to the same prompt are the same answer: equal tokens, or tokens
 * that part, after the first, where both devices chose by a near-tie. The coordinator
 * applies the same rule to two devices' answers to one unit.
 */
export function agree(a: Answer, b: Answer): boolean {
	const n = Math.min(a.tokens.length, b.tokens.length);
	let i = 0;
	while (i < n && a.tokens[i] === b.tokens[i]) i++;
	if (i === a.tokens.length && i === b.tokens.length) return true;
	// The stop token is part of an answer, so one answer ending inside the other is cut
	// short, not the same answer.
	if (i === n) return false;
	return i > 0 && a.margins[i] < NEAR_TIE && b.margins[i] < NEAR_TIE;
}

/** The device cannot compute this model correctly; it should take none of its units. */
export class UnfitError extends Error {
	override name = 'UnfitError';
}

/** Throws `UnfitError` when `got` is not the check's known answer. */
export function assertCheck(modelName: string, check: AiModelCheck, got: Answer): void {
	if (agree({ tokens: check.tokens, margins: check.margins }, got)) return;
	let at = 0;
	while (at < got.tokens.length && got.tokens[at] === check.tokens[at]) at++;
	throw new UnfitError(
		`this device computes ${modelName} wrongly: its answer to the check parts from the known answer at token ${at + 1}`
	);
}
