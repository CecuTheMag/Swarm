import { expect, test } from '@playwright/test';

// Phones turn their screens off after a minute or so, and a hidden page is throttled
// until it stops computing; the worker page keeps the screen on while it takes part.
test('the screen stays on while the device takes part, and not after', async ({
	page,
	context,
	browserName
}) => {
	test.skip(
		browserName === 'webkit' && process.platform === 'linux',
		'WebKit on Linux has no wake lock'
	);
	// Chrome grants it to pages by itself; headless Chromium needs to be told.
	if (browserName === 'chromium') await context.grantPermissions(['screen-wake-lock']);
	await page.goto('/worker');
	await page.getByRole('button', { name: 'Включи се' }).click();
	await expect(page.getByTestId('screen-awake')).toBeVisible();
	const held = () =>
		page.evaluate(async () => {
			// A second lock is granted only while the screen may stay on; release it again.
			const s = await navigator.wakeLock.request('screen');
			await s.release();
			return true;
		});
	expect(await held()).toBe(true);

	await page.getByRole('button', { name: 'Спри' }).click();
	await expect(page.getByTestId('screen-awake')).toHaveCount(0);
});
