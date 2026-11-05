import { describe, expect, it } from 'vitest';
import { toBase64 } from '#lib/bytes.ts';
import type { TtSchool } from '#lib/protocol/TtSchool.ts';
import { brokenRules, csv, decodeSlots, grid, placeAll } from './timetable.ts';

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
		const placed = placeAll(school, slots);
		expect(placed.map((p) => [p.subject, p.day, p.period])).toEqual([
			['Математика', 0, 0],
			['Математика', 1, 1],
			['Физическо', 0, 1]
		]);
		expect(placed[2].room).toBe('Салон');
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
	});
});
