import { readFile } from 'node:fs/promises';
import { describe, expect, it } from 'vitest';
import vectors from '../../../../templates/tt/vectors.json';
import { sha256OfBytes } from '#lib/bytes.ts';
import { solveTimetable, timetableScorer, type TtExports, type TtScoreExports } from '#lib/tt.ts';

// The module the CPU threads run; the Vite config builds it before tests start.
const wasmPath = new URL(
	'../../../../target/wasm32-unknown-unknown/release/kernels_wasm.wasm',
	import.meta.url
);

describe('WebAssembly timetable solver', () => {
	it('gives the shared vectors exactly', async () => {
		const { instance } = await WebAssembly.instantiate(await readFile(wasmPath), {});
		const k = instance.exports as unknown as TtExports;
		expect(vectors.length).toBeGreaterThan(0);
		for (const v of vectors) {
			const school = new TextEncoder().encode(JSON.stringify(v.school));
			const found = solveTimetable(k, school, v.seed, v.index, v.iterations);
			expect(found.cost).toBe(v.cost);
			expect(await sha256OfBytes(found.slots)).toBe(v.slots_sha256);
		}
	});

	it('refuses a school that is not valid', async () => {
		const { instance } = await WebAssembly.instantiate(await readFile(wasmPath), {});
		const k = instance.exports as unknown as TtExports;
		const school = new TextEncoder().encode('{"name": "x"}');
		expect(() => solveTimetable(k, school, 1, 0, 10_000)).toThrow(/not valid/);
	});

	it('scores a timetable as the coordinator does (spec 0015)', async () => {
		const { instance } = await WebAssembly.instantiate(await readFile(wasmPath), {});
		const k = instance.exports as unknown as TtExports & TtScoreExports;
		for (const v of vectors) {
			const school = new TextEncoder().encode(JSON.stringify(v.school));
			const score = timetableScorer(k, school);
			const found = solveTimetable(k, school, v.seed, v.index, v.iterations);
			const view = new DataView(found.slots.buffer);
			const slots = Array.from({ length: found.slots.length / 2 }, (_, i) =>
				view.getUint16(i * 2, true)
			);
			expect(score(slots).total).toBe(v.cost);
		}

		// The tiny school of the Rust tests (crates/timetable/src/lib.rs), and its cases.
		const tiny = {
			name: 'Малко училище',
			days: 2,
			periods: 3,
			teachers: [
				{ name: 'Петрова', unavailable: [{ day: 1, period: 2 }], max_per_day: 3 },
				{ name: 'Иванов', unavailable: [] }
			],
			classes: [
				{ name: '8А', unavailable: [] },
				{ name: '8Б', unavailable: [] }
			],
			rooms: [{ name: 'Физкултурен салон' }],
			lessons: [
				{ class: 0, teacher: 0, subject: 'Математика', per_week: 3, max_per_day: 2 },
				{ class: 1, teacher: 1, subject: 'Физическо', per_week: 2, room: 0, max_per_day: 1 }
			]
		};
		const score = timetableScorer(k, new TextEncoder().encode(JSON.stringify(tiny)));
		expect(score([0, 1, 3, 0, 3]).total).toBe(0);
		const broken = score([0, 0, 5, 1, 2]);
		expect(broken).toEqual({
			total: 3_000_140,
			hard: 3,
			soft: 140,
			breakdown: {
				clashes: 2,
				unavailable: 1,
				pins_moved: 0,
				class_gaps: 0,
				late_starts: 3,
				subject_excess: 1,
				teacher_excess: 0,
				teacher_gaps: 0,
				class_overload: 0
			}
		});
		expect(() => score([0, 1, 2, 3])).toThrow('4 slots for 5');
		expect(() => score([0, 1, 2, 3, 6])).toThrow('hour 4: slot 6 is outside the week');

		// A pinned hour away from its pin is a hard violation; home again, it is not.
		tiny.lessons[1] = { ...tiny.lessons[1], pinned: [{ day: 1, period: 0 }] } as never;
		const pinned = timetableScorer(k, new TextEncoder().encode(JSON.stringify(tiny)));
		expect(pinned([0, 1, 3, 0, 3]).breakdown.pins_moved).toBe(1);
		expect(pinned([0, 1, 3, 0, 3]).total).toBe(1_000_000);
		expect(pinned([0, 1, 3, 3, 0]).total).toBe(0);

		expect(() => timetableScorer(k, new TextEncoder().encode('{"name": "x"}'))).toThrow(
			/not valid/
		);
	});
});
