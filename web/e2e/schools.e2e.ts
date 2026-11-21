// A school's data in the console (spec 0014): imported from a spreadsheet, corrected,
// kept between visits, searched for on the school's devices, and printed.

import { expect, test, type Page } from '@playwright/test';
import { createNetwork, deleteNetwork } from './submit.ts';

/** The plan as Excel saves it in a Bulgarian locale: `;`, a byte order mark, CRLF. */
const PLAN =
	'﻿Паралелка;Предмет;Учител;Часове седмично;Най-много на ден;Стая\r\n' +
	'8А;Български език и литература;Мария Петрова;4;2;\r\n' +
	'8А;Математика;Иван Петров;4;2;\r\n' +
	'8А;"Физика и астрономия";Елена Иванова;2;1;Лаборатория\r\n' +
	'8А;Физическо възпитание и спорт;Георги Иванов;2;1;Салон\r\n' +
	'8Б;Български език и литература;Мария Петрова;4;2;\r\n' +
	'8Б;Математика;Иван Петров;4;2;\r\n' +
	'8Б;Физика и астрономия;Елена Иванова;2;1;Лаборатория\r\n' +
	'8Б;Физическо възпитание и спорт;Георги Иванов;2;1;Салон\r\n';

/** The row of the teacher, class or room called `name` (names are inputs, to rename). */
async function entry(page: Page, name: string) {
	const names = await page
		.getByTestId('entry-name')
		.evaluateAll((inputs) => inputs.map((i) => (i as HTMLInputElement).value));
	expect(names).toContain(name);
	return page.getByTestId('entry').nth(names.indexOf(name));
}

const upload = (page: Page, name: string, text: string) =>
	page.getByTestId('school-file').setInputFiles({
		name,
		mimeType: 'text/csv',
		buffer: Buffer.from(text, 'utf8')
	});

test('a deputy director imports the plan, corrects it, searches and prints', async ({
	page,
	browser
}) => {
	test.setTimeout(180_000);
	// Leaving with unsaved changes asks first; here, the answer is yes.
	page.on('dialog', (d) => void d.accept());
	const { url, link } = await createNetwork(page, 'ПМГ график');
	await page.goto(`${url}/schools`);
	await expect(page.getByTestId('no-schools')).toBeVisible();

	// A plan with mistakes is not imported; each is said with its line.
	await upload(
		page,
		'грешки.csv',
		'Паралелка;Предмет;Учител;Часове седмично\n8А;Математика;Петров;четири\n8А;;Петров;2\n'
	);
	const errors = page.getByTestId('import-errors');
	await expect(errors).toContainText('Ред 2: часовете седмично трябва да е цяло число');
	await expect(errors).toContainText('Ред 3: празно поле „Предмет“');
	await expect(page.getByTestId('no-schools')).toBeVisible();

	// The real plan opens in the editor, saved.
	await upload(page, 'ПМГ 2026-27.csv', PLAN);
	await page.waitForURL(/\/schools\/[0-9a-f-]{36}$/);
	const schoolUrl = page.url();
	await expect(page.getByTestId('school-name')).toHaveValue('ПМГ 2026-27');
	await expect(page.getByTestId('save-state')).toHaveAttribute('data-dirty', 'false');
	// It opens on the first class; all the plan is one choice away.
	await expect(page.getByTestId('lesson')).toHaveCount(4);
	await page.getByTestId('class-filter').selectOption('всички');
	await expect(page.getByTestId('lesson')).toHaveCount(8);
	await expect(page.getByTestId('no-findings')).toBeVisible();

	// Мария Петрова is away on Wednesdays.
	await page.getByTestId('part-teachers').click();
	const maria = await entry(page, 'Мария Петрова');
	await maria.getByTestId('pick-away').click();
	await maria.getByTestId('away-day-2').click();
	await expect(maria.getByTestId('away-2-0')).toHaveAttribute('aria-pressed', 'true');
	await expect(maria.getByTestId('pick-away')).toHaveText('Отсъствия: 7 часа');
	await expect(page.getByTestId('save-state')).toHaveAttribute('data-dirty', 'true');

	// An hour too many for 8А's week is a problem before any search.
	await page.getByTestId('part-plan').click();
	await page.getByTestId('lesson-class').fill('8А');
	await page.getByTestId('lesson-subject').fill('Информационни технологии');
	await page.getByTestId('lesson-teacher').fill('Нов учител');
	await page.getByTestId('lesson-per-week').fill('30');
	await page.getByTestId('lesson-max').fill('6');
	await page.getByTestId('lesson-submit').click();
	await expect(page.getByTestId('lesson')).toHaveCount(9);
	await expect(page.getByTestId('problem')).toContainText('8А има 42 часа седмично');
	await expect(page.getByTestId('school-search')).toBeDisabled();
	await page.getByTestId('remove-lesson').last().click();
	await expect(page.getByTestId('problem')).toHaveCount(0);
	// The teacher it added stays until removed; removing a teacher who teaches is refused.
	await page.getByTestId('part-teachers').click();
	await page.getByTestId('entry').first().getByTestId('remove-entry').click();
	await expect(page.getByTestId('school-error')).toContainText('още има часове');
	await page.getByTestId('entry').last().getByTestId('remove-entry').click();
	await expect(page.getByTestId('entry')).toHaveCount(4);

	await page.getByTestId('save-school').click();
	await expect(page.getByTestId('save-state')).toHaveAttribute('data-dirty', 'false');

	// Found as left, after a reload.
	await page.reload();
	await page.getByTestId('part-teachers').click();
	await expect(page.getByTestId('entry')).toHaveCount(4);
	await expect((await entry(page, 'Мария Петрова')).getByTestId('pick-away')).toHaveText(
		'Отсъствия: 7 часа'
	);

	// Another admin's save in between is not overwritten.
	const other = await page.context().newPage();
	await other.goto(schoolUrl);
	await other.getByTestId('school-name').fill('ПМГ, I срок');
	await other.getByTestId('school-name').press('Tab');
	await other.getByTestId('save-school').click();
	await expect(other.getByTestId('save-state')).toHaveAttribute('data-dirty', 'false');
	await other.close();
	await page.getByTestId('school-name').fill('Моето име');
	await page.getByTestId('school-name').press('Tab');
	await page.getByTestId('save-school').click();
	await expect(page.getByTestId('school-error')).toContainText('version 3');
	await page.reload();
	await expect(page.getByTestId('school-name')).toHaveValue('ПМГ, I срок');

	// The school's device searches; the timetable keeps Мария Петрова's Wednesday free.
	await page.getByTestId('school-searches').fill('2');
	await page.getByTestId('school-quorum').selectOption('1');
	await page.getByTestId('school-search').click();
	await page.waitForURL(/\/jobs$/);
	const job = page.getByTestId('job').first();
	await expect(job).toContainText('Разписание');
	const device = await (await browser.newContext()).newPage();
	await device.goto(`${link}?background`);
	await device.getByRole('button', { name: 'Включи се' }).click();
	await expect(job).toHaveAttribute('data-status', 'done', { timeout: 120_000 });
	await job.getByTestId('timetable-link').click();
	// WebKit on a starved CI runner has run the page's scripts seconds late (run 42278907355).
	await expect(page.getByTestId('timetable-valid')).toHaveAttribute('data-valid', 'true', {
		timeout: 30_000
	});
	await page.getByTestId('view-teachers').click();
	await page.getByTestId('timetable-pick').selectOption({ label: 'Мария Петрова' });
	const wednesday = page.getByTestId('timetable-grid').locator('tbody tr td:nth-of-type(3)');
	await expect(wednesday).toHaveCount(7);
	await expect(wednesday.getByTestId('hour')).toHaveCount(0);

	// Printed: every class on its own page, and nothing of the console on paper.
	await page.evaluate(() => {
		window.print = () => {};
	});
	await page.getByTestId('print-classes').click();
	await page.emulateMedia({ media: 'print' });
	const sheets = page.getByTestId('print-sheet');
	await expect(sheets).toHaveCount(2);
	await expect(sheets.first()).toBeVisible();
	await expect(sheets.first()).toContainText('Паралелка 8А');
	await expect(sheets.nth(1)).toContainText('Паралелка 8Б');
	// 8А's twelve hours a week.
	await expect(sheets.first().getByTestId('hour')).toHaveCount(12);
	await expect(page.getByTestId('network-title')).toBeHidden();
	await page.emulateMedia({ media: 'screen' });
	await expect(sheets.first()).toBeHidden();
	await expect(page.getByTestId('network-title')).toBeVisible();

	// Clean up: later tests start with an empty network list.
	await device.context().close();
	await deleteNetwork(page, url);
});
