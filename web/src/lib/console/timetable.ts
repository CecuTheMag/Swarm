// A timetable as people read it (spec 0013): per class or per teacher, days by periods.

import { fromBase64 } from '#lib/bytes.ts';
import type { TtBreakdown } from '#lib/protocol/TtBreakdown.ts';
import type { TtSchool } from '#lib/protocol/TtSchool.ts';

export const DAY_NAMES = [
	'Понеделник',
	'Вторник',
	'Сряда',
	'Четвъртък',
	'Петък',
	'Събота',
	'Неделя'
];

/** One lesson hour placed in a timetable. */
export interface Placed {
	lesson: number;
	subject: string;
	class: string;
	teacher: string;
	/** Indices into the school's lists: names are for reading, not for telling apart. */
	classIndex: number;
	teacherIndex: number;
	room: string | null;
	day: number;
	period: number;
}

/** The slots of a timetable from their base64 (one little-endian `u16` each). */
export function decodeSlots(base64: string): number[] {
	const bytes = fromBase64(base64);
	const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
	return Array.from({ length: bytes.length / 2 }, (_, i) => view.getUint16(i * 2, true));
}

/** Every lesson hour with its slot, in lesson order, as the solver numbers them. */
export function placeAll(school: TtSchool, slots: number[]): Placed[] {
	const placed: Placed[] = [];
	school.lessons.forEach((l, lesson) => {
		for (let i = 0; i < l.per_week; i++) {
			const slot = slots[placed.length];
			placed.push({
				lesson,
				subject: l.subject,
				class: school.classes[l.class].name,
				teacher: school.teachers[l.teacher].name,
				classIndex: l.class,
				teacherIndex: l.teacher,
				room: l.room == null ? null : school.rooms[l.room].name,
				day: Math.floor(slot / school.periods),
				period: slot % school.periods
			});
		}
	});
	if (placed.length !== slots.length) throw new Error('the timetable does not fit the school');
	return placed;
}

export type View = { kind: 'class' | 'teacher'; index: number };

/** The grid of one class or teacher: `grid[period][day]`, the hours placed there. */
export function grid(school: TtSchool, placed: Placed[], view: View): Placed[][][] {
	const cells: Placed[][][] = Array.from({ length: school.periods }, () =>
		Array.from({ length: school.days }, () => [])
	);
	for (const p of placed) {
		if ((view.kind === 'class' ? p.classIndex : p.teacherIndex) === view.index) {
			cells[p.period][p.day].push(p);
		}
	}
	return cells;
}

const csvField = (v: string) => (/[",\n;]/.test(v) ? `"${v.replaceAll('"', '""')}"` : v);

/** The timetable as CSV for a spreadsheet: one row per lesson hour. */
export function csv(school: TtSchool, placed: Placed[]): string {
	const head = ['Ден', 'Час', 'Паралелка', 'Предмет', 'Учител', 'Стая'];
	const rows = [...placed]
		.sort(
			(a, b) =>
				a.class.localeCompare(b.class, 'bg', { numeric: true }) ||
				a.day - b.day ||
				a.period - b.period
		)
		.map((p) => [
			DAY_NAMES[p.day] ?? String(p.day + 1),
			String(p.period + 1),
			p.class,
			p.subject,
			p.teacher,
			p.room ?? ''
		]);
	return [head, ...rows].map((r) => r.map(csvField).join(',')).join('\n') + '\n';
}

/** The rules a timetable breaks, said plainly, with how often: only those broken. */
export function brokenRules(b: TtBreakdown): { label: string; count: number; hard: boolean }[] {
	const rules: [keyof TtBreakdown, string, boolean][] = [
		['clashes', 'застъпвания (учител, клас или стая на две места)', true],
		['unavailable', 'часове, когато учителят или класът не може', true],
		['class_gaps', 'прозорци за учениците', false],
		['late_starts', 'късно започващи дни', false],
		['subject_excess', 'твърде много от един предмет в ден', false],
		['teacher_excess', 'твърде дълги дни за учители', false],
		['teacher_gaps', 'прозорци за учителите', false],
		['class_overload', 'неравномерни дни', false]
	];
	return rules
		.filter(([key]) => b[key] > 0)
		.map(([key, label, hard]) => ({ label, count: b[key], hard }));
}
