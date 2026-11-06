import { execFileSync } from 'node:child_process';
import { expect, test, type Page } from '@playwright/test';
import { signIn, UNIT_TIMEOUT } from './submit.ts';

// Spec 0010 in the browser: a client creates a network, adds devices with its link,
// sees them live and operates them.

/** A device that opens a join link in its own browser and starts. */
async function joinAsDevice(browser: import('@playwright/test').Browser, link: string) {
	const context = await browser.newContext();
	const device = await context.newPage();
	await device.goto(link);
	await device.getByRole('button', { name: 'Включи се' }).click();
	return device;
}

const status = (p: Page) => p.getByTestId('status');

test('a client creates a network, adds a device by its link and operates it', async ({
	page,
	browser
}, testInfo) => {
	test.setTimeout(UNIT_TIMEOUT + 120_000);
	await signIn(page);
	await page.getByTestId('new-network-name').fill('Кабинет 12');
	await page.getByRole('button', { name: 'Създай' }).click();
	await expect(page.getByTestId('network-title')).toHaveText('Кабинет 12');
	const networkUrl = page.url();

	// The QR code a phone scans leads to this network's join page.
	const link = await page.getByTestId('join-link').inputValue();
	expect(link).toMatch(/\/join\/[a-z2-9]{10}$/);
	const code = await page.getByTestId('join-code').textContent();
	expect(link.endsWith(`/join/${code}`)).toBe(true);
	const qr = page.getByTestId('join-qr');
	await expect(qr).toHaveAttribute('data-url', link);
	const shot = testInfo.outputPath('qr.png');
	await qr.screenshot({ path: shot });
	expect(execFileSync('zbarimg', ['--quiet', '--raw', shot]).toString().trim()).toBe(link);

	// The device sees its network, live.
	const device = await joinAsDevice(browser, link);
	await expect(status(device)).toHaveAttribute('data-status', /waiting|computing/, {
		timeout: 30_000
	});
	await expect(device.getByTestId('network-name')).toHaveText('Кабинет 12');
	await expect(device.getByTestId('network-devices')).toHaveText('1');
	await expect(page.getByTestId('overview-online')).toHaveText('1');

	// Paused by the owner: the device says so; resumed: it carries on.
	await page.goto(`${networkUrl}/devices`);
	const row = page.getByTestId('device-row');
	await expect(row).toHaveAttribute('data-online', 'true');
	await row.getByTestId('toggle-device').click();
	await expect(device.getByTestId('owner-notice')).toHaveAttribute(
		'data-control',
		'paused_by_owner'
	);
	// It finishes the unit in hand first.
	await expect(status(device)).toHaveAttribute('data-status', 'held', { timeout: UNIT_TIMEOUT });
	await row.getByTestId('toggle-device').click();
	await expect(device.getByTestId('owner-notice')).toHaveCount(0);

	// A job of this network runs on its device.
	await page.goto(`${networkUrl}/jobs`);
	await page.getByRole('button', { name: 'Числото π' }).click();
	await page.getByText('Подробни настройки').click();
	await page.getByLabel('Единици').fill('4');
	await page.getByLabel('Проби на единица').fill('65536');
	await page.getByTestId('start-job').click();
	await expect(page.getByTestId('job').first()).toHaveAttribute('data-status', 'done', {
		timeout: 45_000
	});
	await expect(device.getByTestId('network-mine')).not.toHaveText('0');

	// Approval: a second device waits until an admin lets it in.
	await page.goto(`${networkUrl}/rules`);
	await page.getByTestId('join-approval').click();
	await page.getByTestId('save-rules').click();
	await expect(page.getByText('Запазено')).toBeVisible();
	// A lab PC without a camera types the code, as people write it down.
	const newcomerContext = await browser.newContext();
	const newcomer = await newcomerContext.newPage();
	await newcomer.goto('/join');
	await newcomer.getByTestId('join-code-input').fill('nosuchcode');
	await newcomer.getByRole('button', { name: 'Продължи' }).click();
	await expect(newcomer.getByRole('alert')).toContainText('Няма мрежа');
	await newcomer
		.getByTestId('join-code-input')
		.fill(` ${code!.toUpperCase().replace(/(.{5})/, '$1-')} `);
	await newcomer.getByRole('button', { name: 'Продължи' }).click();
	await newcomer.waitForURL(`**/join/${code}`);
	await newcomer.getByRole('button', { name: 'Включи се' }).click();
	await expect(newcomer.getByTestId('owner-notice')).toHaveAttribute(
		'data-control',
		'pending_approval'
	);
	await page.goto(`${networkUrl}/devices`);
	await page.getByTestId('approve').click();
	await expect(newcomer.getByTestId('owner-notice')).toHaveCount(0);

	// A viewer, invited by link, sees the network but cannot change it.
	await page.goto(`${networkUrl}/members`);
	await page.getByTestId('create-invite').click();
	const invite = await page.getByTestId('invite-link').inputValue();
	const viewerContext = await browser.newContext();
	const viewer = await viewerContext.newPage();
	await viewer.goto(invite);
	await viewer.getByTestId('invite-name').fill('Учител');
	await viewer.getByTestId('invite-email').fill(`teacher-${Date.now()}@school.bg`);
	await viewer.getByTestId('invite-password').fill('a teacher password');
	await viewer.getByRole('button', { name: 'Създай профил' }).click();
	await expect(viewer.getByTestId('network-title')).toHaveText('Кабинет 12');
	await expect(viewer.getByTestId('pause-network')).toHaveCount(0);

	// Removed: the device stops and is told why.
	await page.goto(`${networkUrl}/devices`);
	const first = page.getByTestId('device-row').first();
	await first.getByTestId('remove-device').click();
	await first.getByTestId('remove-device').click();
	const removed = [device, newcomer];
	await expect
		.poll(async () => {
			const states = await Promise.all(removed.map((d) => status(d).getAttribute('data-status')));
			return states.filter((s) => s === 'stopped').length;
		})
		.toBe(1);

	// Clean up: the network and its devices go, so later tests start empty.
	await page.goto(`${networkUrl}/rules`);
	await page.getByTestId('delete-network').click();
	await page.getByTestId('delete-network').click();
	await page.waitForURL(/\/app$/);
	await viewerContext.close();
	await device.context().close();
	await newcomer.context().close();
});
