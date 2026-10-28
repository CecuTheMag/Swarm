import { expect, test } from '@playwright/test';

test('the kernel matches the Rust reference bit for bit', async ({ page }, testInfo) => {
	await page.goto('/selftest');
	const status = page.getByTestId('selftest-status');
	await expect(status).toHaveAttribute('data-result', 'pass', { timeout: 45_000 });
	await expect(status).toHaveAttribute(
		'data-backend',
		testInfo.project.name === 'cpu' ? 'wasm' : 'webgpu'
	);
});

test('?backend=wasm tests the CPU path even with a GPU', async ({ page }) => {
	await page.goto('/selftest?backend=wasm');
	const status = page.getByTestId('selftest-status');
	await expect(status).toHaveAttribute('data-result', 'pass', { timeout: 45_000 });
	await expect(status).toHaveAttribute('data-backend', 'wasm');
});
