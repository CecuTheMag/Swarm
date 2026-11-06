import { describe, expect, it } from 'vitest';
import vectors from '../../../../templates/tt/vectors.json';
import type { TtSchool } from '#lib/protocol/TtSchool.ts';
import {
	add,
	addLesson,
	emptySchool,
	findOrAdd,
	inputOf,
	remove,
	removeLesson,
	rename,
	setWeek,
	togglePin,
	toggleUnavailable,
	updateLesson,
	type LessonInput
} from './edit.ts';

const example = (): TtSchool => structuredClone((vectors as { school: TtSchool }[])[2].school);

const lesson = (more: Partial<LessonInput> = {}): LessonInput => ({
	class: '8А',
	subject: 'Математика',
	teacher: 'Петрова',
	per_week: 4,
	max_per_day: 2,
	room: '',
	...more
});

describe('editing a school', () => {
	it('adds lessons, and the classes, teachers and rooms they name', () => {
		const s = emptySchool('ПМГ');
		expect(addLesson(s, lesson())).toBe(0);
		expect(addLesson(s, lesson({ subject: 'Физика', teacher: 'Колев', room: 'Лаборатория' }))).toBe(
			1
		);
		// The same names, typed differently, are the same entries.
		addLesson(s, lesson({ class: ' 8а ', teacher: 'ПЕТРОВА', subject: 'Геометрия' }));
		expect(s.classes.map((c) => c.name)).toEqual(['8А']);
		expect(s.teachers.map((t) => t.name)).toEqual(['Петрова', 'Колев']);
		expect(s.rooms).toEqual([{ name: 'Лаборатория' }]);
		expect(s.lessons[1]).toEqual({
			class: 0,
			teacher: 1,
			subject: 'Физика',
			per_week: 4,
			room: 0,
			max_per_day: 2
		});
		expect(inputOf(s, 1)).toEqual(
			lesson({ subject: 'Физика', teacher: 'Колев', room: 'Лаборатория' })
		);
		updateLesson(s, 1, lesson({ subject: 'Физика', teacher: 'Колев', room: '' }));
		expect(s.lessons[1].room).toBeUndefined();
		removeLesson(s, 0);
		expect(s.lessons.map((l) => l.subject)).toEqual(['Физика', 'Геометрия']);
	});

	it('refuses a lesson it cannot keep, and adds nothing for it', () => {
		const s = emptySchool('ПМГ', 5, 6);
		const before = structuredClone(s);
		for (const [bad, message] of [
			[{ subject: ' ' }, 'Предметът е празен'],
			[{ per_week: 0 }, 'от 1 до 30'],
			[{ per_week: 31 }, 'от 1 до 30'],
			[{ per_week: 2.5 }, 'от 1 до 30'],
			[{ max_per_day: 7 }, 'от 1 до 6'],
			[{ teacher: '' }, 'Името е празно'],
			[{ room: 'x'.repeat(81) }, 'над 80'],
			[{ class: 'x'.repeat(81) }, 'над 80']
		] as const) {
			expect(() => addLesson(s, lesson(bad as Partial<LessonInput>))).toThrow(message);
			expect(s).toEqual(before);
		}
	});

	it('renames everywhere at once, and refuses a name taken', () => {
		const s = example();
		const hers = s.lessons.filter((l) => l.teacher === 0).length;
		rename(s, 'teacher', 0, '  Мария   Петрова-Колева ');
		expect(s.teachers[0].name).toBe('Мария Петрова-Колева');
		expect(s.lessons.filter((l) => l.teacher === 0)).toHaveLength(hers);
		expect(() => rename(s, 'teacher', 1, 'мария петрова-колева')).toThrow('Вече има учител');
		// Its own name, in other letters, is fine.
		rename(s, 'teacher', 0, 'МАРИЯ ПЕТРОВА-КОЛЕВА');
		expect(() => add(s, 'class', s.classes[0].name)).toThrow('Вече има паралелка');
		expect(() => add(s, 'room', '')).toThrow('празно');
		expect(findOrAdd(s, 'room', s.rooms[1].name.toUpperCase())).toBe(1);
	});

	it('removes only what no lesson needs, and renumbers the lessons after it', () => {
		const s = example();
		expect(() => remove(s, 'teacher', 0)).toThrow(/^„.+“ още има часове: .+\. Първо/);
		expect(() => remove(s, 'room', 0)).toThrow('още има часове');
		const unused = add(s, 'teacher', 'Нов учител');
		const later = add(s, 'teacher', 'Последен учител');
		addLesson(
			s,
			lesson({ class: s.classes[0].name, teacher: 'Последен учител', subject: 'Религия' })
		);
		remove(s, 'teacher', unused);
		expect(s.teachers.map((t) => t.name)).not.toContain('Нов учител');
		expect(s.lessons.at(-1)!.teacher).toBe(later - 1);
		expect(s.teachers[s.lessons.at(-1)!.teacher].name).toBe('Последен учител');

		const room = add(s, 'room', 'Празна стая');
		remove(s, 'room', room);
		expect(s.rooms.map((r) => r.name)).not.toContain('Празна стая');
		// Lessons with rooms before it keep theirs.
		expect(s.lessons.filter((l) => l.room != null).every((l) => l.room! < s.rooms.length)).toBe(
			true
		);
	});

	it('marks hours away and back, kept in order', () => {
		const s = example();
		s.teachers[0].unavailable = [];
		s.classes[1].unavailable = [];
		toggleUnavailable(s, 'teacher', 0, { day: 3, period: 1 });
		toggleUnavailable(s, 'teacher', 0, { day: 0, period: 5 });
		toggleUnavailable(s, 'class', 1, { day: 2, period: 0 });
		expect(s.teachers[0].unavailable).toEqual([
			{ day: 0, period: 5 },
			{ day: 3, period: 1 }
		]);
		expect(s.classes[1].unavailable).toEqual([{ day: 2, period: 0 }]);
		toggleUnavailable(s, 'teacher', 0, { day: 3, period: 1 });
		expect(s.teachers[0].unavailable).toEqual([{ day: 0, period: 5 }]);
	});

	it('changes the week, forgetting what falls outside it', () => {
		const s = example();
		s.teachers[0].unavailable = [];
		toggleUnavailable(s, 'teacher', 0, { day: 4, period: 6 });
		toggleUnavailable(s, 'teacher', 0, { day: 1, period: 1 });
		s.teachers[0].max_per_day = 6;
		setWeek(s, 4, 5);
		expect([s.days, s.periods]).toEqual([4, 5]);
		expect(s.teachers[0].unavailable).toEqual([{ day: 1, period: 1 }]);
		expect(s.teachers[0].max_per_day).toBe(5);
		expect(s.lessons.every((l) => l.max_per_day <= 5)).toBe(true);
		expect(() => setWeek(s, 0, 5)).toThrow('Дните');
		expect(() => setWeek(s, 5, 13)).toThrow('Часовете');
		expect([s.days, s.periods]).toEqual([4, 5]);
	});

	it('pins hours and unpins them, up to the hours a week (spec 0015)', () => {
		const s = emptySchool('ПМГ');
		addLesson(s, lesson({ per_week: 2 }));
		togglePin(s, 0, { day: 2, period: 1 });
		togglePin(s, 0, { day: 0, period: 3 });
		expect(s.lessons[0].pinned).toEqual([
			{ day: 0, period: 3 },
			{ day: 2, period: 1 }
		]);
		expect(() => togglePin(s, 0, { day: 4, period: 0 })).toThrow(
			'Математика има 2 часа седмично и всички са закрепени'
		);
		expect(() => togglePin(s, 0, { day: 5, period: 0 })).toThrow('извън седмицата');
		togglePin(s, 0, { day: 2, period: 1 });
		togglePin(s, 0, { day: 0, period: 3 });
		// No pins, no `pinned`: the school reads as one from before pins.
		expect('pinned' in s.lessons[0]).toBe(false);
	});

	it('keeps pins through a change of the lesson and of the week, where they fit', () => {
		const s = emptySchool('ПМГ');
		addLesson(s, lesson({ per_week: 3 }));
		for (const slot of [
			{ day: 0, period: 0 },
			{ day: 1, period: 6 },
			{ day: 4, period: 2 }
		])
			togglePin(s, 0, slot);
		updateLesson(s, 0, lesson({ per_week: 3, teacher: 'Колев' }));
		expect(s.lessons[0].pinned?.length).toBe(3);
		updateLesson(s, 0, lesson({ per_week: 2 }));
		expect(s.lessons[0].pinned).toEqual([
			{ day: 0, period: 0 },
			{ day: 1, period: 6 }
		]);
		setWeek(s, 5, 6);
		expect(s.lessons[0].pinned).toEqual([{ day: 0, period: 0 }]);
		setWeek(s, 5, 1);
		expect(s.lessons[0].pinned).toEqual([{ day: 0, period: 0 }]);
		setWeek(s, 1, 1);
		togglePin(s, 0, { day: 0, period: 0 });
		expect('pinned' in s.lessons[0]).toBe(false);
	});
});
