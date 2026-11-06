import { describe, expect, it } from 'vitest';
import vectors from '../../../../templates/tt/vectors.json';
import type { TtSchool } from '#lib/protocol/TtSchool.ts';
import { check } from './checks.ts';

const examples = (vectors as { school: TtSchool }[]).map((v) => v.school);

/** Two days of three periods; 8А has 5 hours, Петрова teaches 4 of them. */
const school = (): TtSchool => ({
	name: 'Малко училище',
	days: 2,
	periods: 3,
	teachers: [
		{ name: 'Петрова', unavailable: [] },
		{ name: 'Колев', unavailable: [] }
	],
	classes: [{ name: '8А', unavailable: [] }],
	rooms: [{ name: 'Салон' }],
	lessons: [
		{ class: 0, teacher: 0, subject: 'Математика', per_week: 4, max_per_day: 2 },
		{ class: 0, teacher: 1, subject: 'Физическо', per_week: 1, room: 0, max_per_day: 1 }
	]
});

describe('school checks', () => {
	it('find nothing wrong with the example schools', () => {
		for (const s of examples) expect(check(s)).toEqual([]);
		expect(check(school())).toEqual([]);
	});

	it('find a class with more hours than its week', () => {
		const s = school();
		s.classes[0].unavailable = [{ day: 0, period: 0 }];
		s.lessons[1].per_week = 2;
		expect(check(s)).toEqual([
			{
				level: 'problem',
				about: { kind: 'class', index: 0 },
				text: '8А има 6 часа седмично, а седмицата ѝ има 5 часа.'
			}
		]);
	});

	it('find a teacher with more hours than they are there', () => {
		const s = school();
		// Away on the second day: three periods left for four hours.
		s.teachers[0].unavailable = [0, 1, 2].map((period) => ({ day: 1, period }));
		expect(check(s)).toEqual([
			{
				level: 'problem',
				about: { kind: 'teacher', index: 0 },
				text: 'Петрова преподава 4 часа седмично, а е на разположение 3 часа.'
			}
		]);
		// The same slot twice, or a slot outside the week, takes nothing more away.
		s.teachers[0].unavailable = [
			{ day: 1, period: 0 },
			{ day: 1, period: 0 },
			{ day: 5, period: 0 }
		];
		expect(check(s)).toEqual([]);
	});

	it('warn of a teacher who must teach more a day than they wish', () => {
		const s = school();
		s.teachers[0].max_per_day = 1;
		expect(check(s)).toEqual([
			{
				level: 'warning',
				about: { kind: 'teacher', index: 0 },
				text: 'Петрова преподава 4 часа седмично: повече от 1 на ден в някои дни.'
			}
		]);
	});

	it('find a room needed more than the week has', () => {
		const s = school();
		s.lessons[0].room = 0;
		s.lessons[1].per_week = 3;
		s.lessons[1].max_per_day = 2;
		const found = check(s).filter((f) => f.about.kind === 'room');
		expect(found).toEqual([
			{
				level: 'problem',
				about: { kind: 'room', index: 0 },
				text: 'Салон трябва за 7 часа седмично, а седмицата има 6 часа.'
			}
		]);
	});

	it('warn of a subject more often a week than its daily limit allows', () => {
		const s = school();
		s.lessons[0].max_per_day = 1;
		expect(check(s)).toEqual([
			{
				level: 'warning',
				about: { kind: 'lesson', index: 0 },
				text: '8А, Математика: 4 часа седмично при най-много 1 на ден: някои дни ще имат повече.'
			}
		]);
	});

	it('find pinned hours the school itself rules out (spec 0015)', () => {
		const s = school();
		s.teachers[0].unavailable = [{ day: 1, period: 2 }];
		s.classes[0].unavailable = [{ day: 0, period: 2 }];
		s.lessons[0].pinned = [
			{ day: 1, period: 2 },
			{ day: 0, period: 2 },
			{ day: 0, period: 0 }
		];
		s.lessons[1].pinned = [{ day: 0, period: 0 }];
		expect(check(s).map((f) => [f.level, f.about, f.text])).toEqual([
			[
				'problem',
				{ kind: 'lesson', index: 0 },
				'8А, Математика: закрепен за вторник, 3. час, когато Петрова отсъства.'
			],
			[
				'problem',
				{ kind: 'lesson', index: 0 },
				'8А, Математика: закрепен за понеделник, 3. час, когато паралелката няма часове.'
			],
			[
				'problem',
				{ kind: 'lesson', index: 1 },
				'8А има 2 закрепени часа в понеделник, 1. час: 8А, Математика и 8А, Физическо.'
			]
		]);
		// A teacher and a room are found as the class is.
		const t = school();
		t.classes.push({ name: '8Б', unavailable: [] });
		t.lessons.push({
			class: 1,
			teacher: 0,
			subject: 'Математика',
			per_week: 1,
			room: 0,
			max_per_day: 1
		});
		t.lessons[1].pinned = [{ day: 1, period: 1 }];
		t.lessons[2].pinned = [{ day: 1, period: 1 }];
		t.lessons[0].pinned = [{ day: 1, period: 1 }];
		expect(check(t).map((f) => f.text)).toEqual([
			'Петрова има 2 закрепени часа във вторник, 2. час: 8А, Математика и 8Б, Математика.',
			'8А има 2 закрепени часа във вторник, 2. час: 8А, Математика и 8А, Физическо.',
			'Салон има 2 закрепени часа във вторник, 2. час: 8А, Физическо и 8Б, Математика.'
		]);
	});
});
