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
