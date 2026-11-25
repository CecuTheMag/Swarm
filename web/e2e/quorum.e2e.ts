import { expect, test } from '@playwright/test';
import { createNetwork, deleteNetwork } from './submit.ts';

// One device cannot make two results agree. A job that asks for two stalls just before
// the end, and the console says why; the same job that asks for one result finishes, and
// the owner can accept the results that wait.

test('a job that needs two devices says so when one is online; one result finishes', async ({
	page,
	browser
}) => {
	test.setTimeout(150_000);
	await createNetwork(page, 'Две устройства');
	const networkUrl = page.url();
	const link = await page.getByTestId('join-link').inputValue();

	const device = await (await browser.newContext()).newPage();
	await device.goto(`${link}?background`);
	await device.getByRole('button', { name: 'Включи се' }).click();
	await expect(device.getByTestId('status')).toHaveAttribute('data-status', /waiting|computing/, {
		timeout: 30_000
	});

	await page.goto(`${networkUrl}/jobs`);
	// A π job: quick on any device. Two results must agree before a unit counts.
	await page.getByRole('button', { name: 'Числото π' }).click();
	await page.locator('details summary').click();
	await page.getByTestId('job-check').selectOption('2');
	await page.getByTestId('start-job').click();
	const stalled = page.getByTestId('job').first();
	await expect(stalled.getByTestId('needs-more')).toContainText('2 устройства, а на линия има 1', {
		timeout: 20_000
	});
	await expect(stalled).not.toHaveAttribute('data-status', 'done');

	// The same job, with one result per unit, finishes on this one device.
	await page.getByTestId('job-check').selectOption('1');
	await page.getByTestId('start-job').click();
	const finished = page.getByTestId('job').first();
	await expect(finished).toHaveAttribute('data-status', 'done', { timeout: 60_000 });
	await expect(finished.getByTestId('needs-more')).toHaveCount(0);
	// The stalled job still waits, and still says why.
	const waiting = page.getByTestId('job').nth(1);
	await expect(waiting).not.toHaveAttribute('data-status', 'done');

	// The owner can accept what the one device answered: two taps, then the job is done.
	await waiting.getByTestId('accept-unchecked').click();
	await waiting.getByTestId('accept-unchecked').click();
	await expect(waiting).toHaveAttribute('data-status', 'done', { timeout: 30_000 });
	await expect(waiting.getByTestId('needs-more')).toHaveCount(0);

	await deleteNetwork(page, networkUrl);
});
