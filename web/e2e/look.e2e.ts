import { expect, test } from '@playwright/test';

// The glass surfaces blur what scrolls under them in every engine. The CSS minifier once
// kept only `-webkit-backdrop-filter` of the pair, which Chromium ignores: the header
// turned see-through on Android phones.
test('glass surfaces blur what is under them', async ({ page }) => {
	await page.goto('/');
	const header = page.locator('header.bar .inner');
	await expect(header).toBeVisible();
	const blur = await header.evaluate((e) => getComputedStyle(e).backdropFilter);
	expect(blur).toContain('blur');
});
