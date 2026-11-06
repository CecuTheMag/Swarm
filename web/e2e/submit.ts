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
export async function signIn(page: import('@playwright/test').Page, who = ADMIN) {
	await page.goto('/login');
	await page.getByTestId('login-email').fill(who.email);
	await page.getByTestId('login-password').fill(who.password);
	await page.getByRole('button', { name: 'Влез' }).click();
	await page.waitForURL(/\/app$/);
}
