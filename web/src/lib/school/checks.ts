// What makes a school's timetable impossible, or a rule of it unreachable, said before
// a search starts (spec 0014). These are counts only: a school that passes can still have
// no valid timetable, and the search then reports its conflicts.

import type { TtSchool } from '#lib/protocol/TtSchool.ts';

export interface Finding {
	/** `problem`: no valid timetable exists. `warning`: a preference cannot be met. */
	level: 'problem' | 'warning';
	/** Where to look: a class, teacher or room by its place in its list. */
	about: { kind: 'class' | 'teacher' | 'room' | 'lesson'; index: number };
	text: string;
}

const hours = (n: number) => (n === 1 ? '1 час' : `${n} часа`);

export function check(school: TtSchool): Finding[] {
	const week = school.days * school.periods;
	const inWeek = (s: { day: number; period: number }) =>
		s.day < school.days && s.period < school.periods;
	const free = (unavailable: { day: number; period: number }[]) =>
		week - new Set(unavailable.filter(inWeek).map((s) => s.day * school.periods + s.period)).size;
	const sum = (pick: (l: TtSchool['lessons'][number]) => boolean) =>
		school.lessons.filter(pick).reduce((n, l) => n + l.per_week, 0);
	const found: Finding[] = [];

	school.classes.forEach((c, index) => {
		const need = sum((l) => l.class === index);
		const can = free(c.unavailable);
		if (need > can) {
			found.push({
				level: 'problem',
				about: { kind: 'class', index },
				text: `${c.name} има ${hours(need)} седмично, а седмицата ѝ има ${hours(can)}.`
			});
		}
	});

	school.teachers.forEach((t, index) => {
		const need = sum((l) => l.teacher === index);
		const can = free(t.unavailable);
		if (need > can) {
			found.push({
				level: 'problem',
				about: { kind: 'teacher', index },
				text: `${t.name} преподава ${hours(need)} седмично, а е на разположение ${hours(can)}.`
			});
		} else if (t.max_per_day != null && need > t.max_per_day * school.days) {
			found.push({
				level: 'warning',
				about: { kind: 'teacher', index },
				text: `${t.name} преподава ${hours(need)} седмично: повече от ${t.max_per_day} на ден в някои дни.`
			});
		}
	});

	school.rooms.forEach((r, index) => {
		const need = sum((l) => l.room === index);
		if (need > week) {
			found.push({
				level: 'problem',
				about: { kind: 'room', index },
				text: `${r.name} трябва за ${hours(need)} седмично, а седмицата има ${hours(week)}.`
			});
		}
	});

	school.lessons.forEach((l, index) => {
		if (l.per_week > l.max_per_day * school.days) {
			found.push({
				level: 'warning',
				about: { kind: 'lesson', index },
				text:
					`${school.classes[l.class]?.name ?? '?'}, ${l.subject}: ${hours(l.per_week)} седмично ` +
					`при най-много ${l.max_per_day} на ден: някои дни ще имат повече.`
			});
		}
	});
	return found;
}
