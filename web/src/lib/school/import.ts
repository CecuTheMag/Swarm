// A school from the plan a deputy director keeps in a spreadsheet (spec 0014): one row
// per class, subject and teacher. Classes, teachers and rooms come from the names.

import { DAY_SHORT } from '#lib/console/timetable.ts';
import type { TtSchool } from '#lib/protocol/TtSchool.ts';
import type { TtSlot } from '#lib/protocol/TtSlot.ts';
import { parseCsv, writeCsv } from './csv.ts';

/** The columns, and the headers each is known by (compared ignoring case and spaces). */
const COLUMNS = {
	class: ['паралелка', 'клас', 'class'],
	subject: ['предмет', 'subject'],
	teacher: ['учител', 'преподавател', 'teacher'],
	perWeek: ['часове седмично', 'часове', 'часа седмично', 'часа', 'hours'],
	maxPerDay: ['най-много на ден', 'на ден', 'max per day'],
	room: ['стая', 'кабинет', 'room'],
	pinned: ['закрепени', 'закрепени часове', 'pinned']
} as const;
type Column = keyof typeof COLUMNS;
const REQUIRED: Column[] = ['class', 'subject', 'teacher', 'perWeek'];

/** The headers `exportPlan` writes and the import expects first. */
export const PLAN_HEADERS = [
	'Паралелка',
	'Предмет',
	'Учител',
	'Часове седмично',
	'Най-много на ден',
	'Стая',
	'Закрепени'
];

export const DEFAULT_DAYS = 5;
export const DEFAULT_PERIODS = 7;

export type Imported = { school: TtSchool } | { errors: string[] };

const key = (s: string) => s.trim().replace(/\s+/g, ' ').toLocaleLowerCase('bg');

/** Most hours of a subject a day, if the plan does not say: spread over the week. */
export const defaultMaxPerDay = (perWeek: number, days: number) =>
	Math.max(1, Math.ceil(perWeek / days));

function count(text: string, what: string, max: number): number | string {
	const t = text.trim();
	if (!/^\d+$/.test(t)) return `${what} трябва да е цяло число, а е „${text}“`;
	const n = Number(t);
	if (n < 1 || n > max) return `${what} трябва да е от 1 до ${max}, а е ${n}`;
	return n;
}

/** Pinned hours as a plan writes them (spec 0015): „Пн1 Ср3“, Monday's first period and
 * Wednesday's third. */
export const pinsText = (pins: TtSlot[] | undefined) =>
	(pins ?? []).map((s) => `${DAY_SHORT[s.day]}${s.period + 1}`).join(' ');

const PIN = /(пн|вт|ср|чт|пт|сб|нд)\.?\s*(\d{1,2})/g;

/** The pins `text` names, within the week and at most `perWeek`; or why it cannot be read. */
function pinsOf(text: string, perWeek: number, days: number, periods: number): TtSlot[] | string {
	const t = key(text);
	if (t.replace(PIN, '').replace(/[\s,;]+/g, '') !== '') {
		return `„Закрепени“ не се чете: „${text.trim()}“ (пример: Пн1 Ср3)`;
	}
	const pins: TtSlot[] = [];
	for (const [whole, day, period] of t.matchAll(PIN)) {
		const slot = {
			day: DAY_SHORT.findIndex((d) => d.toLocaleLowerCase('bg') === day),
			period: Number(period) - 1
		};
		const shown = whole.replace(/\s+/g, '');
		if (slot.day >= days || slot.period < 0 || slot.period >= periods) {
			return `закрепеният час „${shown}“ е извън седмицата`;
		}
		if (pins.some((s) => s.day === slot.day && s.period === slot.period)) {
			return `„${shown}“ е закрепен два пъти`;
		}
		pins.push(slot);
	}
	if (pins.length > perWeek) {
		return `закрепени са ${pins.length} часа, а седмично са ${perWeek}`;
	}
	return pins.sort((a, b) => a.day - b.day || a.period - b.period);
}

/**
 * The school a plan describes, with the week given; or every reason it cannot be read,
 * each with its line. Nothing is imported unless every row reads.
 */
export function importPlan(
	text: string,
	name: string,
	days = DEFAULT_DAYS,
	periods = DEFAULT_PERIODS
): Imported {
	let rows;
	try {
		rows = parseCsv(text);
	} catch (e) {
		return { errors: [(e as Error).message] };
	}
	if (rows.length === 0) return { errors: ['Файлът е празен.'] };
	const [head, ...body] = rows;
	const at: Partial<Record<Column, number>> = {};
	head.fields.forEach((h, i) => {
		const found = (Object.keys(COLUMNS) as Column[]).find((c) =>
			(COLUMNS[c] as readonly string[]).includes(key(h))
		);
		if (found && at[found] === undefined) at[found] = i;
	});
	const missing = REQUIRED.filter((c) => at[c] === undefined);
	if (missing.length > 0) {
		const names = missing.map((c) => `„${PLAN_HEADERS[REQUIRED.indexOf(c)]}“`);
		return {
			errors: [`Ред ${head.line}: липсват колони ${names.join(', ')}. Първият ред са заглавията.`]
		};
	}
	if (body.length === 0) return { errors: ['Няма редове след заглавията.'] };

	const school: TtSchool = {
		name,
		days,
		periods,
		teachers: [],
		classes: [],
		rooms: [],
		lessons: []
	};
	const index = {
		class: new Map<string, number>(),
		teacher: new Map<string, number>(),
		room: new Map<string, number>()
	};
	const intern = (kind: 'class' | 'teacher' | 'room', value: string) => {
		const k = key(value);
		let i = index[kind].get(k);
		if (i === undefined) {
			const v = value.trim().replace(/\s+/g, ' ');
			if (kind === 'class') i = school.classes.push({ name: v, unavailable: [] }) - 1;
			else if (kind === 'teacher') i = school.teachers.push({ name: v, unavailable: [] }) - 1;
			else i = school.rooms.push({ name: v }) - 1;
			index[kind].set(k, i);
		}
		return i;
	};
	const seen = new Map<string, number>();
	const errors: string[] = [];
	for (const row of body) {
		const cell = (c: Column) => (at[c] === undefined ? '' : (row.fields[at[c]!] ?? ''));
		const problems: string[] = [];
		for (const c of REQUIRED) {
			if (cell(c).trim() === '')
				problems.push(`празно поле „${PLAN_HEADERS[REQUIRED.indexOf(c)]}“`);
		}
		const tooLong = (['class', 'subject', 'teacher', 'room'] as Column[]).find(
			(c) => cell(c).trim().length > 80
		);
		if (tooLong) problems.push('име над 80 знака');
		const perWeek =
			cell('perWeek').trim() === ''
				? null
				: count(cell('perWeek'), 'часовете седмично', days * periods);
		if (typeof perWeek === 'string') problems.push(perWeek);
		const maxText = cell('maxPerDay');
		const maxPerDay =
			maxText.trim() === ''
				? defaultMaxPerDay(typeof perWeek === 'number' ? perWeek : 1, days)
				: count(maxText, 'най-много на ден', periods);
		if (typeof maxPerDay === 'string') problems.push(maxPerDay);
		const pinned =
			typeof perWeek === 'number' ? pinsOf(cell('pinned'), perWeek, days, periods) : [];
		if (typeof pinned === 'string') problems.push(pinned);
		if (problems.length > 0) {
			errors.push(`Ред ${row.line}: ${problems.join('; ')}.`);
			continue;
		}
		const same = [key(cell('class')), key(cell('subject')), key(cell('teacher'))].join('\u0000');
		const earlier = seen.get(same);
		if (earlier !== undefined) {
			errors.push(`Ред ${row.line}: повтаря ред ${earlier} (същите паралелка, предмет и учител).`);
			continue;
		}
		seen.set(same, row.line);
		const room = cell('room').trim();
		school.lessons.push({
			class: intern('class', cell('class')),
			teacher: intern('teacher', cell('teacher')),
			subject: cell('subject').trim().replace(/\s+/g, ' '),
			per_week: perWeek as number,
			...(room === '' ? {} : { room: intern('room', room) }),
			max_per_day: maxPerDay as number,
			...((pinned as TtSlot[]).length === 0 ? {} : { pinned: pinned as TtSlot[] })
		});
	}
	return errors.length > 0 ? { errors } : { school };
}

/** The school's plan as a spreadsheet reads it, and as `importPlan` reads it back. */
export function exportPlan(school: TtSchool): string {
	return writeCsv([
		PLAN_HEADERS,
		...school.lessons.map((l) => [
			school.classes[l.class].name,
			l.subject,
			school.teachers[l.teacher].name,
			String(l.per_week),
			String(l.max_per_day),
			l.room == null ? '' : school.rooms[l.room].name,
			pinsText(l.pinned)
		])
	]);
}

/**
 * A school from a file a deputy director picked: a plan as CSV, or a school as JSON
 * (spec 0013), which is taken as it is and checked when saved.
 */
export function importFile(fileName: string, text: string): Imported {
	const name = fileName.replace(/\.[^.]+$/, '').trim() || 'Училище';
	if (/\.json$/i.test(fileName) || text.trimStart().startsWith('{')) {
		try {
			const school = JSON.parse(text) as TtSchool;
			if (typeof school !== 'object' || school === null || !Array.isArray(school.lessons)) {
				return { errors: ['Файлът не описва училище.'] };
			}
			return { school };
		} catch {
			return { errors: ['Файлът не е валиден JSON.'] };
		}
	}
	return importPlan(text, name);
}
