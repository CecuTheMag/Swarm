import { expect, test } from '@playwright/test';

// Spec 0007: the owner picks how hard the device works, and a hidden page pauses.
test('the power level is remembered, and hiding the page pauses the device', async ({ page }) => {
	await page.goto('/worker');
	await expect(page.getByTestId('power-75')).toHaveAttribute('aria-pressed', 'true');
	await page.getByTestId('power-50').click();
	await expect(page.getByTestId('power-50')).toHaveAttribute('aria-pressed', 'true');
	await page.reload();
	await expect(page.getByTestId('power-50')).toHaveAttribute('aria-pressed', 'true');

	await page.getByRole('button', { name: 'Включи се' }).click();
	await expect(page.getByTestId('status')).toHaveAttribute('data-status', /waiting|computing/, {
		timeout: 30_000
	});

	const setHidden = (hidden: boolean) =>
		page.evaluate((hidden) => {
			Object.defineProperty(document, 'visibilityState', {
				configurable: true,
				get: () => (hidden ? 'hidden' : 'visible')
			});
			document.dispatchEvent(new Event('visibilitychange'));
		}, hidden);

	await setHidden(true);
	await expect(page.getByTestId('status')).toHaveAttribute('data-status', 'paused');
	await expect(page.getByTestId('paused')).toHaveAttribute('data-reason', 'hidden');

	await setHidden(false);
	await expect(page.getByTestId('status')).toHaveAttribute('data-status', /waiting|computing/);
	await expect(page.getByTestId('paused')).toHaveCount(0);
});

test('?background keeps computing while the page is hidden', async ({ page }) => {
	await page.goto('/worker?background');
	await page.getByRole('button', { name: 'Включи се' }).click();
	await expect(page.getByTestId('status')).toHaveAttribute('data-status', /waiting|computing/, {
		timeout: 30_000
	});
	await page.evaluate(() => {
		Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => 'hidden' });
		document.dispatchEvent(new Event('visibilitychange'));
	});
	await page.waitForTimeout(500);
	await expect(page.getByTestId('status')).toHaveAttribute('data-status', /waiting|computing/);
	await expect(page.getByTestId('paused')).toHaveCount(0);
});
