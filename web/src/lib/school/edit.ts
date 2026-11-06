// Changes a deputy director makes to a school in the editor (spec 0014). Each changes
// the school in place (the editor's state) or throws a message to show, leaving it as it
// was. Lessons refer to classes, teachers and rooms by their place in the lists, so
// removing one renumbers the lessons after it.

import type { TtSchool } from '#lib/protocol/TtSchool.ts';
import type { TtSlot } from '#lib/protocol/TtSlot.ts';

export type Kind = 'class' | 'teacher' | 'room';

export const MAX_NAME = 80;

const LIST = { class: 'classes', teacher: 'teachers', room: 'rooms' } as const;
const WHAT = { class: 'паралелка', teacher: 'учител', room: 'стая' } as const;

const key = (s: string) => s.trim().replace(/\s+/g, ' ').toLocaleLowerCase('bg');

function names(school: TtSchool, kind: Kind): string[] {
	return school[LIST[kind]].map((e) => e.name);
}

/** A tidy name for `kind`, or a message why it cannot be one. */
function checked(school: TtSchool, kind: Kind, name: string, except?: number): string {
	const tidy = name.trim().replace(/\s+/g, ' ');
	if (tidy === '') throw new Error('Името е празно.');
	if (tidy.length > MAX_NAME) throw new Error(`Името е над ${MAX_NAME} знака.`);
	const same = names(school, kind).findIndex((n, i) => i !== except && key(n) === key(tidy));
	if (same >= 0) throw new Error(`Вече има ${WHAT[kind]} „${names(school, kind)[same]}“.`);
	return tidy;
}

/** Adds a class, teacher or room; its index. */
export function add(school: TtSchool, kind: Kind, name: string): number {
	const n = checked(school, kind, name);
	if (kind === 'room') return school.rooms.push({ name: n }) - 1;
	if (kind === 'class') return school.classes.push({ name: n, unavailable: [] }) - 1;
	return school.teachers.push({ name: n, unavailable: [] }) - 1;
}

/** The index of the entry called `name`, adding it if there is none. */
export function findOrAdd(school: TtSchool, kind: Kind, name: string): number {
	const i = names(school, kind).findIndex((n) => key(n) === key(name));
	return i >= 0 ? i : add(school, kind, name);
}

/** Renames an entry; lessons follow, as they refer to it by place. */
export function rename(school: TtSchool, kind: Kind, index: number, name: string) {
	school[LIST[kind]][index].name = checked(school, kind, name, index);
}

const uses = (school: TtSchool, kind: Kind, index: number) =>
	school.lessons.filter((l) => (kind === 'room' ? l.room === index : l[kind] === index));

/** Removes an entry no lesson needs; otherwise says which lessons do. */
export function remove(school: TtSchool, kind: Kind, index: number) {
	const used = uses(school, kind, index);
	if (used.length > 0) {
		const shown = used
			.slice(0, 3)
			.map((l) => `${school.classes[l.class].name} ${l.subject}`)
			.join(', ');
		const more = used.length > 3 ? ` и още ${used.length - 3}` : '';
		throw new Error(
			`„${school[LIST[kind]][index].name}“ още има часове: ${shown}${more}. Първо ги махнете или сменете.`
		);
	}
	school[LIST[kind]].splice(index, 1);
	for (const l of school.lessons) {
		if (kind === 'room') {
			if (l.room != null && l.room > index) l.room--;
		} else if (l[kind] > index) {
			l[kind]--;
		}
	}
}

const sameSlot = (a: TtSlot, b: TtSlot) => a.day === b.day && a.period === b.period;

/** Marks a slot unavailable for a class or teacher, or available again. */
export function toggleUnavailable(
	school: TtSchool,
	kind: 'class' | 'teacher',
	index: number,
	slot: TtSlot
) {
	const entry = school[LIST[kind]][index];
	const at = entry.unavailable.findIndex((s) => sameSlot(s, slot));
	if (at >= 0) entry.unavailable.splice(at, 1);
	else entry.unavailable.push({ day: slot.day, period: slot.period });
	entry.unavailable.sort((a, b) => a.day - b.day || a.period - b.period);
}

/** Changes the week; slots outside it are forgotten, limits kept within it. */
export function setWeek(school: TtSchool, days: number, periods: number) {
	if (!Number.isInteger(days) || days < 1 || days > 7) throw new Error('Дните са от 1 до 7.');
	if (!Number.isInteger(periods) || periods < 1 || periods > 12) {
		throw new Error('Часовете на ден са от 1 до 12.');
	}
	school.days = days;
	school.periods = periods;
	const inWeek = (s: TtSlot) => s.day < days && s.period < periods;
	for (const e of [...school.teachers, ...school.classes])
		e.unavailable = e.unavailable.filter(inWeek);
	for (const t of school.teachers) {
		if (t.max_per_day != null && t.max_per_day > periods) t.max_per_day = periods;
	}
	for (const l of school.lessons) l.max_per_day = Math.min(l.max_per_day, periods);
}

export interface LessonInput {
	class: string;
	subject: string;
	teacher: string;
	per_week: number;
	max_per_day: number;
	/** Empty for none. */
	room: string;
}

function lessonOf(school: TtSchool, input: LessonInput) {
	const subject = input.subject.trim().replace(/\s+/g, ' ');
	if (subject === '') throw new Error('Предметът е празен.');
	if (subject.length > MAX_NAME) throw new Error(`Предметът е над ${MAX_NAME} знака.`);
	const week = school.days * school.periods;
	if (!Number.isInteger(input.per_week) || input.per_week < 1 || input.per_week > week) {
		throw new Error(`Часовете седмично са от 1 до ${week}.`);
	}
	if (
		!Number.isInteger(input.max_per_day) ||
		input.max_per_day < 1 ||
		input.max_per_day > school.periods
	) {
		throw new Error(`Най-много на ден е от 1 до ${school.periods}.`);
	}
	// Names are checked before anything is added, so a refused lesson adds nothing.
	for (const kind of ['class', 'teacher', 'room'] as const) {
		const name = input[kind].trim();
		if (kind === 'room' && name === '') continue;
		const exists = names(school, kind).some((n) => key(n) === key(name));
		if (!exists) checked(school, kind, name);
	}
	const room = input.room.trim();
	return {
		class: findOrAdd(school, 'class', input.class),
		teacher: findOrAdd(school, 'teacher', input.teacher),
		subject,
		per_week: input.per_week,
		...(room === '' ? {} : { room: findOrAdd(school, 'room', room) }),
		max_per_day: input.max_per_day
	};
}

/** Adds a lesson, adding the class, teacher or room it names if they are new; its index. */
export function addLesson(school: TtSchool, input: LessonInput): number {
	return school.lessons.push(lessonOf(school, input)) - 1;
}

/** Changes a lesson the same way. */
export function updateLesson(school: TtSchool, index: number, input: LessonInput) {
	school.lessons[index] = lessonOf(school, input);
}

export function removeLesson(school: TtSchool, index: number) {
	school.lessons.splice(index, 1);
}

/** A lesson as the editor's form shows it. */
export function inputOf(school: TtSchool, index: number): LessonInput {
	const l = school.lessons[index];
	return {
		class: school.classes[l.class].name,
		subject: l.subject,
		teacher: school.teachers[l.teacher].name,
		per_week: l.per_week,
		max_per_day: l.max_per_day,
		room: l.room == null ? '' : school.rooms[l.room].name
	};
}

/** A new, empty school of a week of `days` by `periods`. */
export function emptySchool(name: string, days = 5, periods = 7): TtSchool {
	return { name, days, periods, teachers: [], classes: [], rooms: [], lessons: [] };
}
