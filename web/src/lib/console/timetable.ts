// A timetable as people read it (spec 0013): per class or per teacher, days by periods.

import { fromBase64, toBase64 } from '#lib/bytes.ts';
import type { TtBreakdown } from '#lib/protocol/TtBreakdown.ts';
import type { TtSchool } from '#lib/protocol/TtSchool.ts';
import type { TtSlot } from '#lib/protocol/TtSlot.ts';

export const DAY_NAMES = [
	'Понеделник',
	'Вторник',
	'Сряда',
	'Четвъртък',
	'Петък',
	'Събота',
	'Неделя'
];

/** Days as a plan writes them: „Пн1“ is Monday's first period. */
export const DAY_SHORT = ['Пн', 'Вт', 'Ср', 'Чт', 'Пт', 'Сб', 'Нд'];

const DAYS_IN = ['понеделник', 'вторник', 'сряда', 'четвъртък', 'петък', 'събота', 'неделя'];

/** When a slot is, inside a sentence: „в понеделник, 1. час“, „във вторник, 3. час“. */
export const atSlot = (day: number, period: number) =>
	`${day === 1 ? 'във' : 'в'} ${dayPeriod(day, period)}`;

/** What a slot is pinned for: „за понеделник, 1. час“. */
export const forSlot = (day: number, period: number) => `за ${dayPeriod(day, period)}`;

const dayPeriod = (day: number, period: number) =>
	`${DAYS_IN[day] ?? `ден ${day + 1}`}, ${period + 1}. час`;

/** One lesson hour placed in a timetable. */
export interface Placed {
	/** Its index in the timetable, as the solver numbers hours. */
	hour: number;
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
	/** Pinned in the school's data (spec 0015): no search moves it, nor does an edit. */
	pinned: boolean;
}

/** The slots of a timetable from their base64 (one little-endian `u16` each). */
export function decodeSlots(base64: string): number[] {
	const bytes = fromBase64(base64);
	const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
	return Array.from({ length: bytes.length / 2 }, (_, i) => view.getUint16(i * 2, true));
}

/** The base64 of `slots`, as the coordinator takes a timetable. */
export function encodeSlots(slots: number[]): string {
	const bytes = new Uint8Array(slots.length * 2);
	const view = new DataView(bytes.buffer);
	slots.forEach((s, i) => view.setUint16(i * 2, s, true));
	return toBase64(bytes);
}

/** Every lesson hour with its slot, in lesson order, as the solver numbers them. */
export function placeAll(school: TtSchool, slots: number[]): Placed[] {
	const placed: Placed[] = [];
	school.lessons.forEach((l, lesson) => {
		for (let i = 0; i < l.per_week; i++) {
			const slot = slots[placed.length];
			placed.push({
				hour: placed.length,
				lesson,
				subject: l.subject,
				class: school.classes[l.class].name,
				teacher: school.teachers[l.teacher].name,
				classIndex: l.class,
				teacherIndex: l.teacher,
				room: l.room == null ? null : school.rooms[l.room].name,
				day: Math.floor(slot / school.periods),
				period: slot % school.periods,
				pinned: i < (l.pinned?.length ?? 0)
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
		['pins_moved', 'закрепени часове не на мястото си', true],
		['class_gaps', 'прозорци за учениците', false],
		['late_starts', 'късно започващи дни', false],
		['subject_excess', 'твърде много от един предмет в ден', false],
		['teacher_excess', 'твърде дълги дни за учители', false],
		['teacher_gaps', 'прозорци за учителите', false],
		['class_overload', 'неравномерни дни', false]
	];
	// Results from before pins existed have no count of them.
	return rules
		.filter(([key]) => (b[key] ?? 0) > 0)
		.map(([key, label, hard]) => ({ label, count: b[key], hard }));
}

const lessonText = (p: Placed) => `${p.class} ${p.subject}`;

/**
 * The hard rules a timetable breaks, each named: who is in two places when, which
 * lesson is where its teacher or class cannot be, which pinned hour was moved.
 */
export function conflicts(school: TtSchool, placed: Placed[]): string[] {
	const out: string[] = [];
	const at = new Map<string, Placed[]>();
	for (const p of placed) {
		const keys = [`t${p.teacherIndex}`, `c${p.classIndex}`];
		const room = school.lessons[p.lesson].room;
		if (room != null) keys.push(`r${room}`);
		for (const k of keys) {
			const key = `${k} ${p.day} ${p.period}`;
			at.set(key, [...(at.get(key) ?? []), p]);
		}
	}
	for (const [key, hours] of at) {
		if (hours.length < 2) continue;
		const when = atSlot(hours[0].day, hours[0].period);
		const first = hours[0];
		if (key.startsWith('t')) {
			out.push(
				`${first.teacher} има ${hours.length} часа ${when}: ${hours.map(lessonText).join(', ')}.`
			);
		} else if (key.startsWith('c')) {
			out.push(
				`${first.class} има ${hours.length} часа ${when}: ${hours.map((h) => h.subject).join(', ')}.`
			);
		} else {
			out.push(
				`${first.room}: ${hours.length} часа едновременно ${when}: ${hours.map(lessonText).join(', ')}.`
			);
		}
	}
	const off = (list: TtSlot[] | undefined, p: Placed) =>
		(list ?? []).some((s) => s.day === p.day && s.period === p.period);
	const firstHour = new Map<number, number>();
	for (const p of placed) if (!firstHour.has(p.lesson)) firstHour.set(p.lesson, p.hour);
	for (const p of placed) {
		const when = atSlot(p.day, p.period);
		if (off(school.teachers[p.teacherIndex].unavailable, p)) {
			out.push(`${lessonText(p)} е ${when}, когато ${p.teacher} отсъства.`);
		}
		if (off(school.classes[p.classIndex].unavailable, p)) {
			out.push(`${lessonText(p)} е ${when}, когато паралелката няма часове.`);
		}
		const pin = p.pinned
			? school.lessons[p.lesson].pinned?.[p.hour - firstHour.get(p.lesson)!]
			: undefined;
		if (pin && (pin.day !== p.day || pin.period !== p.period)) {
			out.push(`${lessonText(p)}: закрепен ${forSlot(pin.day, pin.period)}, а е ${when}.`);
		}
	}
	return out;
}

/** Whose timetable a view shows a placed hour in. */
const shownIn = (p: Placed, view: View) =>
	(view.kind === 'class' ? p.classIndex : p.teacherIndex) === view.index;

/**
 * Hour `hour` put at `day`, `period` of the class or teacher `view` shows: it trades
 * places with the one hour there, if there is exactly one and it is not pinned in the
 * school, or moves there. The slots after and the hours that moved; null if it is
 * there already.
 */
export function drop(
	school: TtSchool,
	placed: Placed[],
	slots: number[],
	hour: number,
	view: View,
	day: number,
	period: number
): { slots: number[]; moved: number[] } | null {
	const to = day * school.periods + period;
	if (slots[hour] === to) return null;
	const there = placed.filter(
		(p) => p.hour !== hour && shownIn(p, view) && p.day === day && p.period === period
	);
	const next = slots.slice();
	next[hour] = to;
	if (there.length === 1 && !there[0].pinned) {
		next[there[0].hour] = slots[hour];
		return { slots: next, moved: [hour, there[0].hour] };
	}
	return { slots: next, moved: [hour] };
}

/**
 * The school with every hour in `pinned` pinned where `slots` has it (spec 0015), for
 * a search that keeps them; refused when two hours of one lesson would be pinned at
 * one slot.
 */
export function withPins(school: TtSchool, slots: number[], pinned: Iterable<number>): TtSchool {
	const next = JSON.parse(JSON.stringify(school)) as TtSchool;
	const lessonOf: number[] = [];
	school.lessons.forEach((l, i) => {
		for (let k = 0; k < l.per_week; k++) lessonOf.push(i);
	});
	for (const h of [...pinned].sort((a, b) => a - b)) {
		const lesson = next.lessons[lessonOf[h]];
		const slot = { day: Math.floor(slots[h] / school.periods), period: slots[h] % school.periods };
		const pins = (lesson.pinned ??= []);
		if (pins.some((s) => s.day === slot.day && s.period === slot.period)) {
			throw new Error(
				`Два часа на ${next.classes[lesson.class].name} ${lesson.subject} са закрепени ${atSlot(slot.day, slot.period)}. Преместете единия.`
			);
		}
		pins.push(slot);
		pins.sort((a, b) => a.day - b.day || a.period - b.period);
	}
	return next;
}
