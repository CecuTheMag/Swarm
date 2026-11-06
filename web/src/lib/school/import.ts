// A school from the plan a deputy director keeps in a spreadsheet (spec 0014): one row
// per class, subject and teacher. Classes, teachers and rooms come from the names.

import type { TtSchool } from '#lib/protocol/TtSchool.ts';
import { parseCsv, writeCsv } from './csv.ts';

/** The columns, and the headers each is known by (compared ignoring case and spaces). */
const COLUMNS = {
	class: ['паралелка', 'клас', 'class'],
	subject: ['предмет', 'subject'],
	teacher: ['учител', 'преподавател', 'teacher'],
	perWeek: ['часове седмично', 'часове', 'часа седмично', 'часа', 'hours'],
	maxPerDay: ['най-много на ден', 'на ден', 'max per day'],
	room: ['стая', 'кабинет', 'room']
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
	'Стая'
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
			max_per_day: maxPerDay as number
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
			l.room == null ? '' : school.rooms[l.room].name
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
