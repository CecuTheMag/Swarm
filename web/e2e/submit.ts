import { expect, type Page } from '@playwright/test';

// The coordinator under test requires a password for submitting jobs, as on the public
// server (SWARM_SUBMIT_TOKEN).
export const SUBMIT_PASSWORD = 'e2e-submit-password';
export const SUBMIT_HEADERS = { authorization: `Bearer ${SUBMIT_PASSWORD}` };

/** The platform admin of the coordinator under test (playwright.config.ts). */
export const ADMIN = { email: 'e2e-admin@swarm.test', password: 'e2e admin password' };
/** The public network, created by migration 0005. */
export const PUBLIC_NETWORK = '00000000-0000-0000-0000-000000000001';

/**
 * How long a device may take to finish the unit in hand before it pauses or stops: a
 * unit is sized for a phone, and the runner's software GPU is slower than most phones.
 */
export const UNIT_TIMEOUT = 60_000;

/** Signs in on the login page and waits for the console. */
export async function signIn(page: Page, who = ADMIN) {
	await page.goto('/login');
	await page.getByTestId('login-email').fill(who.email);
	await page.getByTestId('login-password').fill(who.password);
	await page.getByRole('button', { name: 'Влез' }).click();
	await page.waitForURL(/\/app$/);
}

/**
 * Creates a network from the console's start page, signed in as `who`, and returns its
 * page address and join link. Three requests and a page change: on a starved two-core
 * runner WebKit took longer than the default 5 s to draw the new network (run
 * 43005304755), so the wait is 30 s, as for the console's first load.
 */
export async function createNetwork(page: Page, name: string, who = ADMIN) {
	await signIn(page, who);
	await page.getByTestId('new-network-name').fill(name);
	await page.getByRole('button', { name: 'Създай' }).click();
	await expect(page.getByTestId('network-title')).toHaveText(name, { timeout: 30_000 });
	return { url: page.url(), link: await page.getByTestId('join-link').inputValue() };
}

/**
 * Deletes the network at `networkUrl` from its rules page, as its owner does: a tap that
 * asks, a tap that confirms. The question lasts 4 s; a browser starved on a two-core
 * runner has delivered the second tap after that (Firefox, 4.2 s late), so a tap that
 * came too late is made again.
 */
export async function deleteNetwork(page: Page, networkUrl: string) {
	await page.goto(`${networkUrl}/rules`);
	const button = page.getByTestId('delete-network');
	await expect(async () => {
		if (/\/app$/.test(page.url())) return;
		if (!(await button.textContent({ timeout: 5_000 }))?.includes('окончателно')) {
			await button.click({ timeout: 5_000 });
		}
		await expect(button).toHaveText('Изтрий окончателно', { timeout: 3_000 });
		await button.click({ timeout: 5_000 });
		await page.waitForURL(/\/app$/, { timeout: 15_000 });
	}).toPass({ timeout: 90_000 });
}
