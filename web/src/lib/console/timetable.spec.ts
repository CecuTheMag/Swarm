import { describe, expect, it } from 'vitest';
import { toBase64 } from '#lib/bytes.ts';
import type { TtSchool } from '#lib/protocol/TtSchool.ts';
import type { TtBreakdown } from '#lib/protocol/TtBreakdown.ts';
import {
	brokenRules,
	conflicts,
	csv,
	decodeSlots,
	drop,
	encodeSlots,
	grid,
	placeAll,
	withPins
} from './timetable.ts';

const school: TtSchool = {
	name: 'Малко училище',
	days: 2,
	periods: 3,
	teachers: [
		{ name: 'Петрова', unavailable: [] },
		{ name: 'Иванов, Г.', unavailable: [] }
	],
	classes: [
		{ name: '8А', unavailable: [] },
		{ name: '8Б', unavailable: [] }
	],
	rooms: [{ name: 'Салон' }],
	lessons: [
		{ class: 0, teacher: 0, subject: 'Математика', per_week: 2, max_per_day: 1 },
		{ class: 1, teacher: 1, subject: 'Физическо', per_week: 1, room: 0, max_per_day: 1 }
	]
};

const encode = (slots: number[]) => {
	const bytes = new Uint8Array(slots.length * 2);
	slots.forEach((s, i) => new DataView(bytes.buffer).setUint16(i * 2, s, true));
	return toBase64(bytes);
};

describe('timetables', () => {
	it('decodes slots and places hours in lesson order', () => {
		const slots = decodeSlots(encode([0, 4, 1]));
		expect(slots).toEqual([0, 4, 1]);
		expect(encodeSlots([0, 4, 1, 65_535])).toBe(encode([0, 4, 1, 65_535]));
		expect(decodeSlots(encodeSlots([300, 7]))).toEqual([300, 7]);
		const placed = placeAll(school, slots);
		expect(placed.map((p) => [p.subject, p.day, p.period])).toEqual([
			['Математика', 0, 0],
			['Математика', 1, 1],
			['Физическо', 0, 1]
		]);
		expect(placed[2].room).toBe('Салон');
		expect(placed.map((p) => [p.hour, p.pinned])).toEqual([
			[0, false],
			[1, false],
			[2, false]
		]);
		expect(() => placeAll(school, [0, 1])).toThrow();
	});

	it('draws one class or teacher', () => {
		const placed = placeAll(school, [0, 4, 1]);
		const a = grid(school, placed, { kind: 'class', index: 0 });
		expect(a[0][0].map((p) => p.subject)).toEqual(['Математика']);
		expect(a[1][1].map((p) => p.subject)).toEqual(['Математика']);
		expect(a[1][0]).toEqual([]);
		const ivanov = grid(school, placed, { kind: 'teacher', index: 1 });
		expect(ivanov[1][0].map((p) => p.class)).toEqual(['8Б']);
	});

	it('tells apart a class and a teacher by their place in the list', () => {
		// Two teachers of one name (an older school file): each sees only their own hours.
		const namesakes: TtSchool = {
			...school,
			teachers: [school.teachers[0], { ...school.teachers[0] }]
		};
		const placed = placeAll(namesakes, [0, 4, 1]);
		const first = grid(namesakes, placed, { kind: 'teacher', index: 0 });
		const second = grid(namesakes, placed, { kind: 'teacher', index: 1 });
		expect(first.flat(2).map((p) => p.subject)).toEqual(['Математика', 'Математика']);
		expect(second.flat(2).map((p) => p.subject)).toEqual(['Физическо']);
	});

	it('orders classes as a school does: 8А before 10А', () => {
		const grades: TtSchool = {
			...school,
			classes: [
				{ name: '10А', unavailable: [] },
				{ name: '8А', unavailable: [] }
			]
		};
		const rows = csv(grades, placeAll(grades, [0, 4, 1])).split('\n');
		expect(rows.slice(1, 4).map((r) => r.split(',')[2])).toEqual(['8А', '10А', '10А']);
	});

	it('writes CSV a spreadsheet reads', () => {
		const text = csv(school, placeAll(school, [0, 4, 1]));
		expect(text.split('\n')).toEqual([
			'Ден,Час,Паралелка,Предмет,Учител,Стая',
			'Понеделник,1,8А,Математика,Петрова,',
			'Вторник,2,8А,Математика,Петрова,',
			'Понеделник,2,8Б,Физическо,"Иванов, Г.",Салон',
			''
		]);
	});

	it('names only the rules broken', () => {
		const none = {
			clashes: 0,
			unavailable: 0,
			pins_moved: 0,
			class_gaps: 0,
			late_starts: 0,
			subject_excess: 0,
			teacher_excess: 0,
			teacher_gaps: 0,
			class_overload: 0
		};
		expect(brokenRules(none)).toEqual([]);
		expect(brokenRules({ ...none, clashes: 2, teacher_gaps: 3 })).toEqual([
			{ label: expect.stringContaining('застъпвания'), count: 2, hard: true },
			{ label: 'прозорци за учителите', count: 3, hard: false }
		]);
		expect(brokenRules({ ...none, pins_moved: 1 })).toEqual([
			{ label: 'закрепени часове не на мястото си', count: 1, hard: true }
		]);
		// A result kept from before pins existed has no count of them.
		const before: Partial<TtBreakdown> = { ...none };
		delete before.pins_moved;
		expect(brokenRules(before as TtBreakdown)).toEqual([]);
	});
});

describe('editing a timetable (spec 0015)', () => {
	// Петрова teaches both classes; 8А's first maths hour is pinned at Monday, period 1.
	const pinned: TtSchool = {
		...school,
		teachers: [{ name: 'Петрова', unavailable: [{ day: 1, period: 2 }] }, school.teachers[1]],
		classes: [
			{ name: '8А', unavailable: [] },
			{ name: '8Б', unavailable: [{ day: 1, period: 0 }] }
		],
		lessons: [
			{ ...school.lessons[0], pinned: [{ day: 0, period: 0 }] },
			{ class: 1, teacher: 0, subject: 'Математика', per_week: 1, max_per_day: 1 },
			school.lessons[1]
		]
	};

	it('marks the hours the school pins', () => {
		const placed = placeAll(pinned, [0, 4, 2, 1]);
		expect(placed.map((p) => p.pinned)).toEqual([true, false, false, false]);
	});

	it('names every hard conflict', () => {
		// 8Б's maths at Monday period 1 with Петрова, who has 8А then; 8А's second maths
		// hour on Tuesday's last period, when Петрова is away; PE when 8Б has no lessons.
		expect(conflicts(pinned, placeAll(pinned, [0, 5, 0, 3]))).toEqual([
			'Петрова има 2 часа в понеделник, 1. час: 8А Математика, 8Б Математика.',
			'8А Математика е във вторник, 3. час, когато Петрова отсъства.',
			'8Б Физическо е във вторник, 1. час, когато паралелката няма часове.'
		]);
		// The pinned hour moved off its pin, onto the gym's other booking.
		const moved = conflicts(pinned, placeAll(pinned, [1, 4, 2, 1]));
		expect(moved).toEqual([
			'8А Математика: закрепен за понеделник, 1. час, а е в понеделник, 2. час.'
		]);
		expect(conflicts(pinned, placeAll(pinned, [0, 4, 2, 1]))).toEqual([]);
	});

	it('names a room needed twice at once', () => {
		const gym: TtSchool = {
			...school,
			lessons: [{ ...school.lessons[0], room: 0 }, school.lessons[1]]
		};
		expect(conflicts(gym, placeAll(gym, [1, 4, 1]))).toEqual([
			'Салон: 2 часа едновременно в понеделник, 2. час: 8А Математика, 8Б Физическо.'
		]);
	});

	it('moves an hour to an empty slot, or trades it with the one there', () => {
		const slots = [0, 4, 2, 1];
		const placed = placeAll(pinned, slots);
		const a = { kind: 'class', index: 0 } as const;
		// 8А's second maths hour to Monday period 3, empty for 8А.
		expect(drop(pinned, placed, slots, 1, a, 0, 2)).toEqual({ slots: [0, 2, 2, 1], moved: [1] });
		// Where it is already: nothing to do.
		expect(drop(pinned, placed, slots, 1, a, 1, 1)).toBeNull();
		// Onto 8А's pinned hour: it does not trade places, they clash.
		expect(drop(pinned, placed, slots, 1, a, 0, 0)).toEqual({ slots: [0, 0, 2, 1], moved: [1] });
		// In Петрова's week, 8Б's maths trades places with 8А's second hour.
		const petrova = { kind: 'teacher', index: 0 } as const;
		expect(drop(pinned, placed, slots, 2, petrova, 1, 1)).toEqual({
			slots: [0, 2, 4, 1],
			moved: [2, 1]
		});
	});

	it('pins the hours a person pinned, for the next search', () => {
		const next = withPins(pinned, [0, 4, 2, 1], new Set([2, 1]));
		expect(next.lessons.map((l) => l.pinned)).toEqual([
			[
				{ day: 0, period: 0 },
				{ day: 1, period: 1 }
			],
			[{ day: 0, period: 2 }],
			undefined
		]);
		// The school itself is left as it was.
		expect(pinned.lessons[1].pinned).toBeUndefined();
		expect(pinned.lessons[0].pinned).toEqual([{ day: 0, period: 0 }]);
		// Two hours of one lesson pinned at one slot cannot be.
		expect(() => withPins(pinned, [0, 0, 2, 1], [1])).toThrow(/Два часа на 8А Математика/);
	});
});
