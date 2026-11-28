import { expect, test, type WebSocketRoute } from '@playwright/test';
import type { JobSummary } from '../src/lib/protocol/JobSummary.ts';
import { SUBMIT_HEADERS } from './submit.ts';

// A phone that loses its signal mid-unit: the coordinator takes back the units of the
// lost connection, the page reconnects, and the units of the new connection must be
// computed even though the old unit is still running when they arrive.
let jobId: string | undefined;

// A job left unfinished would hold up every later test's job (jobs run in order).
test.afterEach(async ({ request }) => {
	if (jobId) await request.post(`/api/jobs/${jobId}/cancel`, { headers: SUBMIT_HEADERS });
	jobId = undefined;
});

test('a worker that loses its connection mid-unit carries on after reconnecting', async ({
	page,
	request
}, testInfo) => {
	test.setTimeout(120_000);
	let connections = 0;
	let current: [WebSocketRoute, WebSocketRoute] | undefined;
	await page.routeWebSocket(/\/ws\/worker$/, (ws) => {
		connections++;
		const server = ws.connectToServer();
		ws.onMessage((m) => server.send(m));
		server.onMessage((m) => ws.send(m));
		ws.onClose(() => server.close());
		server.onClose(() => ws.close());
		current = [ws, server];
	});

	await page.goto('/worker');
	await page.getByRole('button', { name: 'Включи се' }).click();
	await expect(page.getByTestId('device')).toHaveAttribute('data-backend', /.+/);

	// Units of about two seconds each (128² pixels × 1024 samples on a CPU), so the
	// connection drops while one is computing and the next ones arrive before it ends.
	// SwiftShader, the software GPU of headless Chromium, is several times slower than
	// the CPU path on a two-core CI runner: an eighth of the samples there.
	const spp = testInfo.project.name === 'webgpu' ? 128 : 1024;
	const res = await request.post('/api/jobs', {
		headers: SUBMIT_HEADERS,
		data: {
			template: 'pt',
			params: { width: 512, height: 128, tile: 128, spp, passes: 1, seed: 77 }
		}
	});
	expect(res.status()).toBe(201);
	const job = (await res.json()) as JobSummary;
	jobId = job.id;

	await expect(page.getByTestId('status')).toHaveAttribute('data-status', 'computing', {
		timeout: 30_000
	});
	await page.waitForTimeout(500);
	// The signal is gone for both ends.
	await Promise.all(current!.map((end) => end.close()));
	await expect.poll(() => connections, { timeout: 15_000 }).toBe(2);

	await expect
		.poll(
			async () => ((await (await request.get(`/api/jobs/${job.id}`)).json()) as JobSummary).status,
			{ timeout: 90_000, intervals: [1000] }
		)
		.toBe('done');

	await page.getByRole('button', { name: 'Спри' }).click();
	await expect(page.getByTestId('status')).toHaveAttribute('data-status', 'stopped');
	// Nothing of the routed connections is left for the next test to count as a worker.
	const home = await page.context().newPage();
	await home.goto('/');
	await expect(home.getByTestId('home-live')).toContainText('на линия');
	// No device is online, and a job has finished: the hero shows the totals so far
	// instead of a zero.
	await expect(home.getByTestId('home-total-units')).toBeVisible();
	await expect(home.getByTestId('home-devices')).toHaveCount(0);
});
