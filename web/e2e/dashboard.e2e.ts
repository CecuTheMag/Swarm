import { execFileSync } from 'node:child_process';
import { expect, test } from '@playwright/test';
import { SUBMIT_PASSWORD } from './submit.ts';

test('only the password starts a job, and the render page invites phones by QR code', async ({
	page
}, testInfo) => {
	await page.goto('/dashboard');
	await expect(page.getByTestId('connection')).toHaveText('Свързано с координатора');
	// A tiny image, so the default 1280×720 job does not stay in the shared queue.
	await page.getByLabel('Ширина').fill('32');
	await page.getByLabel('Височина').fill('18');
	await page.getByLabel('Плочка').fill('16');
	const start = page.getByRole('button', { name: 'Пусни' });

	await start.click();
	await expect(page.getByRole('alert')).toHaveText('Нужна е парола, за да пуснете задача.');

	await page.getByTestId('password').fill('guess');
	await start.click();
	await expect(page.getByRole('alert')).toHaveText('Грешна парола.');

	await page.getByTestId('password').fill(SUBMIT_PASSWORD);
	await start.click();
	await expect(page).toHaveURL(/\/render\/[0-9a-f-]{36}$/);

	// The QR code a phone scans leads to the worker page of this very site.
	const qr = page.getByTestId('join-qr');
	await expect(qr).toHaveAttribute('data-url', new URL('/worker', page.url()).href);
	const shot = testInfo.outputPath('qr.png');
	await qr.screenshot({ path: shot });
	const decoded = execFileSync('zbarimg', ['--quiet', '--raw', shot]).toString().trim();
	expect(decoded).toBe(new URL('/worker', page.url()).href);

	// The password is remembered on this device, and it stops the job too: nothing must
	// queue behind a forgotten job during a demo.
	const jobId = page.url().split('/').at(-1)!;
	await page.goto('/dashboard');
	await expect(page.getByTestId('password')).toHaveValue(SUBMIT_PASSWORD);
	const row = page.locator(`[data-job-id="${jobId}"]`);
	await row.getByTestId('stop-job').click();
	await expect(row).toHaveAttribute('data-status', 'failed');
	await expect(row).toContainText('спряна');
	await expect(row.getByTestId('stop-job')).toHaveCount(0);
});
