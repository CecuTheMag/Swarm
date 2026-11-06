// School timetables (spec 0013): the solver compiled to WebAssembly, as the CPU threads
// and the tests run it.

import { toBase64 } from './bytes.ts';
import type { TtCost } from '#lib/protocol/TtCost.ts';

export interface TtExports {
	memory: WebAssembly.Memory;
	tt_school(len: number): number;
	tt_solve(seed: number, index: number, iterations: number): number;
	tt_slots(): number;
	tt_cost(): number;
}

export interface TtSearch {
	/** Little-endian `u16` slots, one per lesson hour. */
	slots: Uint8Array;
	cost: number;
}

/**
 * Runs search `index` of a job with `seed` over `school` (its JSON bytes). The same
 * search gives the same timetable as on any other device and in the coordinator.
 */
export function solveTimetable(
	k: TtExports,
	school: Uint8Array,
	seed: number,
	index: number,
	iterations: number
): TtSearch {
	// Each call may grow the module's memory, which replaces its buffer: the buffer is
	// read only after the call that returns the pointer.
	const at = k.tt_school(school.length);
	new Uint8Array(k.memory.buffer, at, school.length).set(school);
	const len = k.tt_solve(seed, index, iterations);
	if (len < 0) throw new Error('the school is not valid');
	const slots = k.tt_slots();
	// Copied out: the module's memory is reused by the next search.
	return { slots: new Uint8Array(k.memory.buffer, slots, len).slice(), cost: k.tt_cost() };
}

/** A unit's output as the coordinator takes it. */
export const ttOutput = (s: TtSearch) => ({ slots: toBase64(s.slots), cost: s.cost });

/** The exports that score a timetable a person edits (spec 0015). */
export interface TtScoreExports {
	memory: WebAssembly.Memory;
	tt_school(len: number): number;
	tt_prepare(): number;
	tt_timetable(hours: number): number;
	tt_score(): number;
	tt_out(): number;
}

/**
 * Scores timetables of `school` (its JSON bytes) exactly as the coordinator does, for as
 * long as no other school is prepared in the module.
 */
export function timetableScorer(
	k: TtScoreExports,
	school: Uint8Array
): (slots: ArrayLike<number>) => TtCost {
	// The buffer is read after each call that may grow the memory, as in `solveTimetable`.
	const at = k.tt_school(school.length);
	new Uint8Array(k.memory.buffer, at, school.length).set(school);
	const hours = k.tt_prepare();
	if (hours < 0) throw new Error('the school is not valid');
	return (slots) => {
		if (slots.length !== hours) {
			throw new Error(`${slots.length} slots for ${hours} lesson hours`);
		}
		const slotsAt = k.tt_timetable(hours);
		const view = new DataView(k.memory.buffer, slotsAt, hours * 2);
		for (let i = 0; i < hours; i++) view.setUint16(i * 2, slots[i], true);
		const len = k.tt_score();
		const out = k.tt_out();
		const text = new TextDecoder().decode(new Uint8Array(k.memory.buffer, out, len));
		const score = JSON.parse(text) as { cost: TtCost } | { error: string };
		if ('error' in score) throw new Error(score.error);
		return score.cost;
	};
}
