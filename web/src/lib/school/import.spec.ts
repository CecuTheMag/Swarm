import { describe, expect, it } from 'vitest';
import vectors from '../../../../templates/tt/vectors.json';
import type { TtSchool } from '#lib/protocol/TtSchool.ts';
import { exportPlan, importFile, importPlan } from './import.ts';

const schools = (vectors as { school: TtSchool }[]).map((v) => v.school);

const errorsOf = (text: string) => {
	const r = importPlan(text, 'Училище');
	if (!('errors' in r)) throw new Error('imported');
	return r.errors;
};
const schoolOf = (text: string, days?: number, periods?: number) => {
	const r = importPlan(text, 'Училище', days, periods);
	if ('errors' in r) throw new Error(r.errors.join('\n'));
	return r.school;
};

describe('importing a plan', () => {
	it('builds the school from the names, in the order they first appear', () => {
		const s = schoolOf(
			'﻿Паралелка;Предмет;Учител;Часове седмично;Най-много на ден;Стая\r\n' +
				'8А;Математика;Петрова;4;2;\r\n' +
				'8А;Физика и астрономия;Колев;2;;Лаборатория\r\n' +
				'8Б;Математика;  петрова ;4;1;\r\n' +
				'8б;Физическо възпитание;Маринов;3;;Салон\r\n'
		);
		expect(s.days).toBe(5);
		expect(s.periods).toBe(7);
		expect(s.classes.map((c) => c.name)).toEqual(['8А', '8Б']);
		// The same teacher, however the name was typed.
		expect(s.teachers.map((t) => t.name)).toEqual(['Петрова', 'Колев', 'Маринов']);
		expect(s.rooms.map((r) => r.name)).toEqual(['Лаборатория', 'Салон']);
		expect(s.lessons).toEqual([
			{ class: 0, teacher: 0, subject: 'Математика', per_week: 4, max_per_day: 2 },
			{
				class: 0,
				teacher: 1,
				subject: 'Физика и астрономия',
				per_week: 2,
				room: 0,
				max_per_day: 1
			},
			{ class: 1, teacher: 0, subject: 'Математика', per_week: 4, max_per_day: 1 },
			// Three hours over five days: one a day unless the plan says otherwise.
			{
				class: 1,
				teacher: 2,
				subject: 'Физическо възпитание',
				per_week: 3,
				room: 1,
				max_per_day: 1
			}
		]);
	});

	it('knows the columns by other names, in any order, and ignores the rest', () => {
		const s = schoolOf('Учител,Бележка,Клас,Часове,Предмет\nПетрова,x,10А,6,Математика\n');
		expect(s.lessons).toEqual([
			// Six hours over five days: two on some days.
			{ class: 0, teacher: 0, subject: 'Математика', per_week: 6, max_per_day: 2 }
		]);
		// A six-day week spreads them one a day.
		expect(
			schoolOf('Клас,Предмет,Учител,Часове\n10А,Математика,Петрова,6\n', 6).lessons[0].max_per_day
		).toBe(1);
	});

	it('reports every row it cannot read, by line, and imports nothing', () => {
		const errors = errorsOf(
			'Паралелка;Предмет;Учител;Часове седмично;Най-много на ден\n' +
				'8А;Математика;Петрова;4;2\n' +
				'8А;;Петрова;4;2\n' +
				'\n' +
				'8Б;Химия;Колев;две;1\n' +
				'8Б;Химия;Колев;0;1\n' +
				'8Б;Химия;Колев;2;9\n' +
				'8В;История;Маринов;;\n' +
				'8а;математика;ПЕТРОВА;3;1\n' +
				`8В;${'х'.repeat(81)};Маринов;1;1\n`
		);
		expect(errors).toEqual([
			'Ред 3: празно поле „Предмет“.',
			'Ред 5: часовете седмично трябва да е цяло число, а е „две“.',
			'Ред 6: часовете седмично трябва да е от 1 до 35, а е 0.',
			'Ред 7: най-много на ден трябва да е от 1 до 7, а е 9.',
			'Ред 8: празно поле „Часове седмично“.',
			'Ред 9: повтаря ред 2 (същите паралелка, предмет и учител).',
			'Ред 10: име над 80 знака.'
		]);
	});

	it('says which columns are missing, and when there is nothing to import', () => {
		expect(errorsOf('Клас;Учител\n8А;Петрова\n')).toEqual([
			'Ред 1: липсват колони „Предмет“, „Часове седмично“. Първият ред са заглавията.'
		]);
		expect(errorsOf('')).toEqual(['Файлът е празен.']);
		expect(errorsOf('Клас;Предмет;Учител;Часове\n\n')).toEqual(['Няма редове след заглавията.']);
		expect(errorsOf('Клас;Предмет;Учител;Часове\n8А;"Математика;Петрова;4\n')[0]).toContain(
			'Ред 2'
		);
	});

	it('reads pinned hours as a plan writes them (spec 0015)', () => {
		const school = schoolOf(
			'Клас;Предмет;Учител;Часове;Закрепени\n' +
				'8А;Математика;Петрова;4;Пн1 Ср3\n' +
				'8А;Физика;Колев;2;"пн 2, пт7"\n' +
				'8А;Химия;Колев;2;\n'
		);
		expect(school.lessons.map((l) => l.pinned)).toEqual([
			[
				{ day: 0, period: 0 },
				{ day: 2, period: 2 }
			],
			[
				{ day: 0, period: 1 },
				{ day: 4, period: 6 }
			],
			undefined
		]);
		expect('pinned' in school.lessons[2]).toBe(false);
		expect(exportPlan(school).split('\r\n')[1]).toBe('8А;Математика;Петрова;4;1;;Пн1 Ср3');
	});

	it('says why pinned hours cannot be read', () => {
		expect(
			errorsOf(
				'Клас;Предмет;Учител;Часове;Закрепени\n' +
					'8А;Математика;Петрова;4;Сб1\n' +
					'8Б;Математика;Петрова;4;Пн8\n' +
					'8В;Математика;Петрова;4;Пн0\n' +
					'8Г;Математика;Петрова;4;понеделник\n' +
					'8Д;Математика;Петрова;4;Пн1 пн 1\n' +
					'8Е;Математика;Петрова;2;Пн1 Вт1 Ср1\n'
			)
		).toEqual([
			'Ред 2: закрепеният час „сб1“ е извън седмицата.',
			'Ред 3: закрепеният час „пн8“ е извън седмицата.',
			'Ред 4: закрепеният час „пн0“ е извън седмицата.',
			'Ред 5: „Закрепени“ не се чете: „понеделник“ (пример: Пн1 Ср3).',
			'Ред 6: „пн1“ е закрепен два пъти.',
			'Ред 7: закрепени са 3 часа, а седмично са 2.'
		]);
	});

	it('reads back the plan it exports, for every example school', () => {
		for (const school of schools) {
			const back = schoolOf(exportPlan(school), school.days, school.periods);
			expect(back.lessons).toEqual(school.lessons);
			expect(back.classes.map((c) => c.name)).toEqual(school.classes.map((c) => c.name));
			expect(back.teachers.map((t) => t.name)).toEqual(
				school.teachers
					.filter((_, i) => school.lessons.some((l) => l.teacher === i))
					.map((t) => t.name)
			);
			expect(back.rooms.map((r) => r.name)).toEqual(school.rooms.map((r) => r.name));
		}
	});
});

describe('importing a file', () => {
	it('takes a JSON school as it is, and a CSV plan named after its file', () => {
		const json = JSON.stringify(schools[0]);
		expect(importFile('pmg.json', json)).toEqual({ school: schools[0] });
		const plan = importFile(
			'ПМГ 2026-27.csv',
			'Клас;Предмет;Учител;Часове\n8А;Математика;Петрова;4\n'
		);
		expect('school' in plan && plan.school.name).toBe('ПМГ 2026-27');
		expect(importFile('x.json', '{ nope')).toEqual({ errors: ['Файлът не е валиден JSON.'] });
		expect(importFile('x.json', '[1, 2]')).toEqual({ errors: ['Файлът не описва училище.'] });
		expect(importFile('x.json', 'null')).toEqual({ errors: ['Файлът не описва училище.'] });
	});
});
