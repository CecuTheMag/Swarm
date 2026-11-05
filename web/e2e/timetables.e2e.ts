// School timetables (spec 0013) in the browser: a school's devices search for its
// timetable, and the console shows the best one per class and per teacher.

import { readFileSync } from 'node:fs';
import { expect, test } from '@playwright/test';
import { signIn } from './submit.ts';
import { solveTimetable, type TtExports } from '../src/lib/tt.ts';
import type { JobSummary } from '../src/lib/protocol/JobSummary.ts';
import type { TtJobParams } from '../src/lib/protocol/TtJobParams.ts';
import type { TtResult } from '../src/lib/protocol/TtResult.ts';

/** The solver the devices run, loaded here to check their results independently. */
async function solver(): Promise<TtExports> {
	const wasm = readFileSync(
		new URL('../../target/wasm32-unknown-unknown/release/kernels_wasm.wasm', import.meta.url)
	);
	const { instance } = await WebAssembly.instantiate(wasm, {});
	return instance.exports as unknown as TtExports;
}

test("a school's devices find its timetable and the console shows it", async ({
	page,
	browser
}, testInfo) => {
	test.setTimeout(180_000);
	await signIn(page);
	await page.getByTestId('new-network-name').fill('ПМГ разписание');
	await page.getByRole('button', { name: 'Създай' }).click();
	await expect(page.getByTestId('network-title')).toHaveText('ПМГ разписание');
	const networkUrl = page.url();
	const network = new URL(networkUrl).pathname.split('/').at(-1);
	const link = await page.getByTestId('join-link').inputValue();

	await page.goto(`${networkUrl}/jobs`);
	await page.getByTestId('kind-tt').click();
	await page.getByTestId('tt-classes').selectOption('5');
	await page.getByTestId('tt-quick').click();
	await page.getByTestId('tt-searches').fill('4');
	// One device: it cannot check itself, and cannot fake a better timetable anyway.
	await page.getByTestId('tt-quorum').selectOption('1');
	await page.getByTestId('start-job').click();
	const job = page.getByTestId('job').first();
	await expect(job).toContainText('Разписание');

	const device = await (await browser.newContext()).newPage();
	await device.goto(`${link}?background`);
	await device.getByRole('button', { name: 'Включи се' }).click();
	await expect(job).toHaveAttribute('data-status', 'done', { timeout: 120_000 });

	// Every search's cost is what the solver gives for it, independently of the devices.
	const jobId = (await job.getAttribute('data-job-id'))!;
	const summary = (await (
		await page.request.get(`/api/networks/${network}/jobs/${jobId}`)
	).json()) as JobSummary;
	const params = summary.params as unknown as TtJobParams;
	const result = summary.result as unknown as TtResult;
	const k = await solver();
	const school = new TextEncoder().encode(JSON.stringify(params.school));
	const expected = [0, 1, 2, 3].map(
		(i) => solveTimetable(k, school, params.seed, i, params.iterations).cost
	);
	expect(result.costs).toEqual(expected);
	expect(result.cost.total).toBe(Math.min(...expected));

	await job.getByTestId('timetable-link').click();
	await expect(page.getByTestId('timetable-valid')).toHaveAttribute('data-valid', 'true');
	await expect(page.getByTestId('timetable-cost')).toHaveText(String(result.cost.total));
	// The first class's week holds its 31 hours.
	await expect(page.getByTestId('hour')).toHaveCount(31);
	await page.getByTestId('view-teachers').click();
	await expect(page.getByTestId('hour').first()).toBeVisible();
	await expect(page.getByTestId('timetable-spread').locator('.bar')).toHaveCount(4);

	const download = page.waitForEvent('download');
	await page.getByTestId('save-csv').click();
	const file = testInfo.outputPath('timetable.csv');
	await (await download).saveAs(file);
	const rows = readFileSync(file, 'utf8').trim().split('\n');
	expect(rows[0]).toBe('Ден,Час,Паралелка,Предмет,Учител,Стая');
	// Five classes of 31 hours each.
	expect(rows).toHaveLength(1 + 5 * 31);

	// Clean up: later tests start with an empty network list.
	await device.context().close();
	await page.goto(`${networkUrl}/rules`);
	await page.getByTestId('delete-network').click();
	await page.getByTestId('delete-network').click();
	await page.waitForURL(/\/app$/);
});
