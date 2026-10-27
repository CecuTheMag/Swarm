import { expect, test } from '@playwright/test';

test('WGSL kernel matches the Rust reference bit for bit', async ({ page }) => {
	await page.goto('/selftest');
	await expect(page.getByTestId('selftest-status')).toHaveAttribute('data-result', 'pass', {
		timeout: 45_000
	});
});
