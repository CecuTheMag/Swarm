import { expect, test } from '@playwright/test';
import { SUBMIT_HEADERS, SUBMIT_PASSWORD } from './submit.ts';
import type { JobSummary } from '../src/lib/protocol/JobSummary.ts';

// The chaos panel (spec 0008) on the projector page, against the real coordinator.

test.use({ viewport: { width: 1920, height: 1080 } });

test('the organiser switches a device off and adds a cheater that is caught', async ({
	page,
	context,
	request
}) => {
	test.setTimeout(240_000);
	const job = async (data: object) => {
		const res = await request.post('/api/jobs', { headers: SUBMIT_HEADERS, data });
		expect(res.status()).toBe(201);
		return (await res.json()) as JobSummary;
	};
	const status = async (id: string) =>
		((await (await request.get(`/api/jobs/${id}`)).json()) as JobSummary).status;

	// A browser worker, and a finished job whose units serve as known answers.
	const worker = await context.newPage();
	await worker.goto('/worker');
	await worker.getByRole('button', { name: 'Включи се' }).click();
	const known = await job({
		template: 'pi',
		params: { units: 8, samples_per_unit: 65536, seed: 4242 }
	});
	await expect.poll(() => status(known.id), { timeout: 30_000 }).toBe('done');

	// Small tiles that take the CPU a moment, so the job runs while the panel is used.
	const render = await job({
		template: 'pt',
		params: { width: 320, height: 180, tile: 32, spp: 64, passes: 4, seed: 99 }
	});
	try {
		// Without the password the room's view has no controls.
		await page.goto(`/render/${render.id}`);
		await expect(page.getByTestId('render-devices')).toHaveText('1');
		await expect(page.getByTestId('chaos-panel')).toHaveCount(0);

		await page.evaluate((pw) => localStorage.setItem('swarm-submit-password', pw), SUBMIT_PASSWORD);
		await page.reload();
		const panel = page.getByTestId('chaos-panel');
		await expect(panel).toBeVisible();
		await expect(page.getByTestId('render-devices')).toHaveText('1');

		// Switched off, the browser worker comes back as a new device.
		await page.getByTestId('chaos-kill').click();
		await expect(page.getByTestId('chaos-message')).toContainText('Изключено');
		await expect(page.getByTestId('feed')).toContainText('Изключи се');
		await expect(page.getByTestId('chaos-totals')).toContainText('Изключили се: 1');
		await expect(worker.getByTestId('status')).toHaveAttribute('data-status', /computing|waiting/, {
			timeout: 20_000
		});
		await expect(page.getByTestId('render-devices')).toHaveText('1');

		// A cheater joins, is caught, and the job still finishes.
		await page.getByTestId('chaos-cheater').click();
		await expect(page.getByTestId('chaos-message')).toContainText('Измамник се включва');
		await expect(page.getByTestId('caught-count')).toHaveText('1', { timeout: 60_000 });
		await expect(page.getByTestId('feed')).toContainText('Хванат измамник');
		// One device renders 256 tiles in software: on a slow CI runner 223 were done
		// after a minute (run 42278907355).
		await expect.poll(() => status(render.id), { timeout: 120_000 }).toBe('done');
	} finally {
		await request.post(`/api/jobs/${render.id}/cancel`, { headers: SUBMIT_HEADERS });
		await worker.getByRole('button', { name: 'Спри' }).click();
	}
	// Nothing is left connected for the next test to count.
	await expect(page.getByTestId('render-devices')).toHaveText('0', { timeout: 20_000 });
});
