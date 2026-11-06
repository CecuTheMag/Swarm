// Pinned lessons and a timetable edited by hand (spec 0015): pins entered with the plan
// and in the editor, a found timetable changed with the cost of each move shown first,
// kept, printed, given back, and searched again around the hours pinned.

import { expect, test, type Locator, type Page } from '@playwright/test';
import { signIn } from './submit.ts';

/** Two classes of ten hours; 8А's PE is pinned at Monday's first period. */
const PLAN =
	'Паралелка;Предмет;Учител;Часове седмично;Най-много на ден;Стая;Закрепени\n' +
	'8А;Български език и литература;Мария Петрова;4;2;;\n' +
	'8А;Математика;Иван Петров;4;2;;\n' +
	'8А;Физическо възпитание и спорт;Георги Иванов;2;1;Салон;Пн1\n' +
	'8Б;Български език и литература;Мария Петрова;4;2;;\n' +
	'8Б;Математика;Иван Петров;4;2;;\n' +
	'8Б;Физическо възпитание и спорт;Георги Иванов;2;1;Салон;\n';

/** The cell of `period` and `day` (from 0) in a week's grid. */
const cell = (grid: Locator, period: number, day: number) =>
	grid.locator(`tbody tr:nth-child(${period + 1}) td:nth-of-type(${day + 1})`);

/** The first target whose preview breaks `hard` more rules (or more than 0 for 'any'). */
async function target(grid: Locator, hard: number | 'any') {
	const all = grid.getByTestId('target');
	const found = await all.evaluateAll(
		(els, want) =>
			els.findIndex((e) => {
				const h = Number((e as HTMLElement).dataset.hard);
				return want === 'any' ? h > 0 : h === want;
			}),
		hard
	);
	expect(found).toBeGreaterThanOrEqual(0);
	const t = all.nth(found);
	return {
		t,
		day: Number(await t.getAttribute('data-day')),
		period: Number(await t.getAttribute('data-period'))
	};
}

async function waitForTimetable(page: Page) {
	await expect(page.getByTestId('timetable-valid')).toBeVisible({ timeout: 120_000 });
}

test('a deputy director pins lessons, edits the timetable found, and searches again', async ({
	page,
	browser
}) => {
	test.setTimeout(240_000);
	page.on('dialog', (d) => void d.accept());
	await signIn(page);
	await page.getByTestId('new-network-name').fill('Ръчно разписание');
	await page.getByRole('button', { name: 'Създай' }).click();
	await expect(page.getByTestId('network-title')).toHaveText('Ръчно разписание');
	const url = page.url();
	const link = await page.getByTestId('join-link').inputValue();

	// The plan's pin is read with it.
	await page.goto(`${url}/schools`);
	await page.getByTestId('school-file').setInputFiles({
		name: 'Закрепени.csv',
		mimeType: 'text/csv',
		buffer: Buffer.from(PLAN, 'utf8')
	});
	await page.waitForURL(/\/schools\/[0-9a-f-]{36}$/);
	await expect(page.getByTestId('lesson')).toHaveCount(3);
	await expect(page.getByTestId('lesson-pins').nth(2)).toHaveText('Пн1');

	// 8Б's PE cannot share Monday's first period with 8А's (one teacher, one gym); it
	// is pinned at Tuesday's.
	await page.getByTestId('class-filter').selectOption({ label: '8Б' });
	await page.getByTestId('pin-lesson').nth(2).click();
	await expect(page.getByTestId('pin-0-0')).toBeDisabled();
	await page.getByTestId('pin-1-0').click();
	await expect(page.getByTestId('pin-1-0')).toHaveAttribute('aria-pressed', 'true');
	await expect(page.getByTestId('lesson-pins').nth(2)).toHaveText('Вт1');
	await expect(page.getByTestId('no-findings')).toBeVisible();
	await page.getByTestId('save-school').click();
	await expect(page.getByTestId('save-state')).toHaveAttribute('data-dirty', 'false');

	// The school's device searches; the pinned hours are where they were pinned.
	await page.getByTestId('school-searches').fill('2');
	await page.getByTestId('school-quorum').selectOption('1');
	await page.getByTestId('school-search').click();
	await page.waitForURL(/\/jobs$/);
	const device = await (await browser.newContext()).newPage();
	await device.goto(`${link}?background`);
	await device.getByRole('button', { name: 'Включи се' }).click();
	const job = page.getByTestId('job').first();
	await expect(job).toHaveAttribute('data-status', 'done', { timeout: 120_000 });
	await job.getByTestId('timetable-link').click();
	await waitForTimetable(page);
	await expect(page.getByTestId('timetable-valid')).toHaveAttribute('data-valid', 'true');
	const grid = page.getByTestId('timetable-grid').first();
	await expect(cell(grid, 0, 0)).toContainText(/Физическо възпитание и спорт\s*🔒/);
	await page.getByTestId('timetable-pick').selectOption({ label: '8Б' });
	await expect(cell(grid, 0, 1)).toContainText(/Физическо възпитание и спорт\s*🔒/);
	await page.getByTestId('timetable-pick').selectOption({ label: '8А' });

	// Editing: hours pinned in the school do not move.
	await page.getByTestId('edit-timetable').click();
	await expect(page.getByTestId('edit-toolbar')).toBeVisible();
	await expect(grid.locator('button[data-testid="hour"][disabled]')).toHaveCount(1);
	const maths = grid
		.locator('button[data-testid="hour"]:not([disabled])', { hasText: 'Математика' })
		.first();
	const hour = await maths.getAttribute('data-hour');
	await maths.click();
	await expect(page.getByTestId('edit-help')).toContainText('8А Математика');

	// Where Иван Петров teaches 8Б, 8А's maths breaks a hard rule: said before, and
	// named after.
	const clash = await target(grid, 'any');
	await clash.t.click();
	await expect(page.getByTestId('timetable-valid')).toHaveAttribute('data-valid', 'false');
	await expect(page.getByTestId('conflict').first()).toContainText(/има 2 часа/);
	await expect(page.getByTestId('edit-state')).toHaveAttribute('data-dirty', 'true');
	await page.getByTestId('undo').click();
	await expect(page.getByTestId('timetable-valid')).toHaveAttribute('data-valid', 'true');
	await expect(page.getByTestId('conflict')).toHaveCount(0);
	await expect(page.getByTestId('edit-state')).toHaveCount(0);

	// A move that breaks nothing hard, kept and pinned.
	await grid.locator(`button[data-hour="${hour}"]`).click();
	const free = await target(grid, 0);
	await free.t.click();
	const moved = cell(grid, free.period, free.day).locator(`[data-hour="${hour}"]`);
	await expect(moved).toContainText(/Математика\s*📌/);
	await expect(page.getByTestId('timetable-valid')).toHaveAttribute('data-valid', 'true');
	const live = await page.getByTestId('timetable-cost').textContent();
	await page.getByTestId('save-edit').click();
	await expect(page.getByTestId('edit-state')).toHaveAttribute('data-dirty', 'false');

	// Found as left after a reload, with the coordinator's cost equal to the one shown.
	await page.reload();
	await waitForTimetable(page);
	await expect(page.getByTestId('edit-state')).toHaveText('редактирано');
	await expect(page.getByTestId('timetable-cost')).toHaveText(live!);
	await expect(cell(grid, free.period, free.day).locator(`[data-hour="${hour}"]`)).toContainText(
		/Математика\s*📌/
	);

	// Printed as edited, without the marks.
	await page.evaluate(() => {
		window.print = () => {};
	});
	await page.getByTestId('print-shown').click();
	await page.emulateMedia({ media: 'print' });
	const sheet = page.getByTestId('print-sheet');
	await expect(sheet).toHaveCount(1);
	const printed = cell(sheet.getByTestId('timetable-grid'), free.period, free.day);
	await expect(printed).toContainText('Математика');
	await expect(sheet.getByText('🔒')).toBeHidden();
	await page.emulateMedia({ media: 'screen' });

	// Given back: the timetable found; one step back, the edit again, saved anew.
	await page.getByTestId('edit-timetable').click();
	await page.getByTestId('revert').click();
	await page.getByTestId('revert').click();
	await expect(page.getByTestId('timetable-notice')).toContainText('намереното');
	await expect(page.getByTestId('edit-state')).toHaveCount(0);
	await expect(cell(grid, free.period, free.day).locator(`[data-hour="${hour}"]`)).toHaveCount(0);
	await page.getByTestId('undo').click();
	await expect(page.getByTestId('edit-state')).toHaveAttribute('data-dirty', 'true');
	await page.getByTestId('save-edit').click();
	await expect(page.getByTestId('edit-state')).toHaveAttribute('data-dirty', 'false');

	// Searched again: the hour put by hand is pinned in the new job's school.
	const first = page.url();
	await page.getByTestId('search-again').click();
	await page.waitForURL((u) => u.href !== first && /\/timetable$/.test(u.pathname));
	await waitForTimetable(page);
	await expect(page.getByTestId('timetable-valid')).toHaveAttribute('data-valid', 'true');
	await expect(page.getByTestId('edit-state')).toHaveCount(0);
	await expect(cell(grid, free.period, free.day)).toContainText(/Математика\s*🔒/);
	await expect(cell(grid, 0, 0)).toContainText(/Физическо възпитание и спорт\s*🔒/);

	// Clean up: later tests start with an empty network list.
	await device.context().close();
	await page.goto(`${url}/rules`);
	await page.getByTestId('delete-network').click();
	await page.getByTestId('delete-network').click();
	await page.waitForURL(/\/app$/);
});
