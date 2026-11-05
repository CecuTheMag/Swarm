// Programs (spec 0012) in the browser: a network runs its own WebAssembly program on
// its devices, a device stays usable whatever the program does, and review opens the
// public network.

import { readFileSync } from 'node:fs';
import { expect, test, type Browser, type Page } from '@playwright/test';
import { signIn } from './submit.ts';
import type { JobOutputs } from '../src/lib/protocol/JobOutputs.ts';
import type { JobSummary } from '../src/lib/protocol/JobSummary.ts';

const fixturePath = (name: string) =>
	new URL(`../src/lib/program/fixtures/${name}.wasm`, import.meta.url).pathname;

/** FNV-1a (64-bit) of `input`, little-endian, as base64: what the `fnv` fixture outputs. */
function fnv(input: Uint8Array): string {
	let h = 0xcbf29ce484222325n;
	for (const b of input) h = BigInt.asUintN(64, (h ^ BigInt(b)) * 0x100000001b3n);
	const out = Buffer.alloc(8);
	out.writeBigUInt64LE(h);
	return out.toString('base64');
}

/** Unit `i`'s input for a job given as a count without arguments. */
const indexInput = (i: number) => {
	const b = Buffer.alloc(4);
	b.writeUInt32LE(i);
	return b;
};

/** Creates a network in the console; returns its page address and join link. */
async function createNetwork(page: Page, name: string) {
	await signIn(page);
	await page.getByTestId('new-network-name').fill(name);
	await page.getByRole('button', { name: 'Създай' }).click();
	await expect(page.getByTestId('network-title')).toHaveText(name);
	return { url: page.url(), link: await page.getByTestId('join-link').inputValue() };
}

/** Clean up: later tests start with an empty network list. */
async function deleteNetwork(page: Page, url: string) {
	await page.goto(`${url}/rules`);
	await page.getByTestId('delete-network').click();
	await page.getByTestId('delete-network').click();
	await page.waitForURL(/\/app$/);
}

async function uploadProgram(page: Page, url: string, fixture: string) {
	await page.goto(`${url}/programs`);
	await page.getByTestId('program-name').fill(fixture);
	await page.getByTestId('program-file').setInputFiles(fixturePath(fixture));
	await page.getByTestId('upload').click();
	await expect(page.getByTestId('program')).toHaveCount(1);
}

/** Starts a job of the network's only program, as a count of units, one result each. */
async function startProgramJob(page: Page, url: string, count: number) {
	await page.goto(`${url}/jobs`);
	await page.getByTestId('kind-program').click();
	await page.getByTestId('job-count').fill(String(count));
	// One device in these tests: it cannot check itself.
	await page.getByTestId('job-quorum').selectOption('1');
	await page.getByTestId('start-job').click();
	const job = page.getByTestId('job').first();
	await expect(job).toHaveAttribute('data-status', /queued|running/);
	return job;
}

async function joinAsDevice(browser: Browser, link: string) {
	const device = await (await browser.newContext()).newPage();
	await device.goto(`${link}?background`);
	await device.getByRole('button', { name: 'Включи се' }).click();
	return device;
}

test('a network runs its own program on its devices and downloads the outputs', async ({
	page,
	browser
}, testInfo) => {
	test.setTimeout(120_000);
	const { url, link } = await createNetwork(page, 'Кабинет по физика');
	await uploadProgram(page, url, 'fnv');
	await expect(page.getByTestId('review')).toHaveAttribute('data-review', 'private');

	const job = await startProgramJob(page, url, 20);
	await expect(job).toContainText('fnv');
	const device = await joinAsDevice(browser, link);
	await expect(job).toHaveAttribute('data-status', 'done', { timeout: 60_000 });
	// The device hears its last result accepted just after the job is done.
	await expect
		.poll(async () => Number(await device.getByTestId('accepted').textContent()))
		.toBeGreaterThanOrEqual(20);

	// The outputs are what the program computes, unit by unit.
	const expected = Array.from({ length: 20 }, (_, i) => fnv(indexInput(i)));
	const network = new URL(url).pathname.split('/').at(-1);
	const jobId = await job.getAttribute('data-job-id');
	const res = await page.request.get(`/api/networks/${network}/jobs/${jobId}/outputs`);
	expect(((await res.json()) as JobOutputs).outputs).toEqual(expected);

	const download = page.waitForEvent('download');
	await job.getByTestId('download').click();
	// Saved rather than read in place: the browser may run elsewhere (WebKit's container).
	const file = testInfo.outputPath('outputs.json');
	await (await download).saveAs(file);
	expect((JSON.parse(readFileSync(file, 'utf8')) as JobOutputs).outputs).toEqual(expected);
	await device.context().close();
	await deleteNetwork(page, url);
});

test('a program that never returns leaves the device usable and stops when asked', async ({
	page,
	browser
}) => {
	test.setTimeout(120_000);
	const { url, link } = await createNetwork(page, 'Безкраен цикъл');
	await uploadProgram(page, url, 'endless');
	const job = await startProgramJob(page, url, 1);

	const device = await joinAsDevice(browser, link);
	const status = device.getByTestId('status');
	await expect(status).toHaveAttribute('data-status', 'computing', { timeout: 30_000 });
	// The loop runs in a thread of its own: the page still answers.
	await device.getByTestId('power-50').click();
	await expect(device.getByTestId('power-50')).toHaveAttribute('aria-pressed', 'true');
	await device.getByRole('button', { name: 'Спри' }).click();
	await expect(status).toHaveAttribute('data-status', 'stopped');

	// The unit went back to the queue: starting again gets it again.
	await device.getByRole('button', { name: 'Включи се' }).click();
	await expect(status).toHaveAttribute('data-status', 'computing', { timeout: 30_000 });
	await device.getByRole('button', { name: 'Спри' }).click();
	await expect(job).toHaveAttribute('data-status', 'running');
	await job.getByTestId('stop-job').click();
	await page.getByRole('button', { name: 'Спри задачата?' }).click();
	await expect(job).toHaveAttribute('data-status', 'failed');
	await device.context().close();
	await deleteNetwork(page, url);
});

test('review opens the public network to an approved program', async ({ page }) => {
	const { url } = await createNetwork(page, 'Отворена наука');
	await uploadProgram(page, url, 'fnv');
	await page.getByTestId('ask-review').click();
	await expect(page.getByTestId('review')).toHaveAttribute('data-review', 'pending');

	await page.goto('/app/admin');
	const network = new URL(url).pathname.split('/').at(-1);
	const card = page
		.getByTestId('pending')
		.filter({ hasText: 'Отворена наука' })
		.filter({ hasText: 'fnv' });
	await expect(card).toHaveCount(1);
	await card.getByTestId('approve').click();
	await expect(card).toHaveCount(0);

	await page.goto(`/app/n/${network}/programs`);
	await expect(page.getByTestId('review')).toHaveAttribute('data-review', 'approved');
	await page.goto(`/app/n/${network}/jobs`);
	await page.getByTestId('kind-program').click();
	await expect(page.getByTestId('job-public')).toBeVisible();
	await page.getByTestId('job-public').check();
	await expect(page.getByTestId('job-quorum')).toHaveCount(0);
	await page.getByTestId('job-count').fill('2');
	await page.getByTestId('start-job').click();
	const job = page.getByTestId('job').first();
	await expect(job).toContainText('на общата мрежа');
	// Deleting the network stops its job on the public network too.
	await deleteNetwork(page, url);
	const res = await page.request.get('/api/networks/00000000-0000-0000-0000-000000000001/jobs');
	const jobs = (await res.json()) as JobSummary[];
	expect(jobs.find((j) => j.owner_network_id !== null && j.status !== 'failed')).toBeUndefined();
});

test("a contributor's choice about community programs is kept", async ({ page }) => {
	await page.goto('/worker');
	const community = page.getByTestId('community');
	await expect(community).toBeChecked();
	await community.uncheck();
	await page.reload();
	await expect(community).not.toBeChecked();
	// It applies from the next start, so it is fixed while the device takes part.
	await page.getByRole('button', { name: 'Включи се' }).click();
	await expect(community).toBeDisabled();
	await page.getByRole('button', { name: 'Спри' }).click();
	await community.check();
	await page.reload();
	await expect(community).toBeChecked();
});
