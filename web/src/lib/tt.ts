// School timetables (spec 0013): the solver compiled to WebAssembly, as the CPU threads
// and the tests run it.

import { toBase64 } from './bytes.ts';

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
