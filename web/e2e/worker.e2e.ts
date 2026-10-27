import { expect, test } from '@playwright/test';
import vectors from '../../templates/pi/vectors.json' with { type: 'json' };
import type { JobSummary } from '../src/lib/protocol/JobSummary.ts';
import type { PiResult } from '../src/lib/protocol/PiResult.ts';

test('a browser worker computes a job on the GPU with an exact result', async ({
	page,
	request
}) => {
	await page.goto('/worker');
	await page.getByRole('button', { name: 'Включи се' }).click();
	await expect(page.getByTestId('status')).toHaveAttribute('data-status', /waiting|computing/, {
		timeout: 30_000
	});

	// Units 0 and 1 of seed 42 are pinned test vectors, so the exact answer is known.
	const res = await request.post('/api/jobs', {
		data: { template: 'pi', params: { units: 2, samples_per_unit: 65536, seed: 42 } }
	});
	expect(res.status()).toBe(201);
	const job = (await res.json()) as JobSummary;

	await expect
		.poll(
			async () => ((await (await request.get(`/api/jobs/${job.id}`)).json()) as JobSummary).status,
			{
				timeout: 45_000
			}
		)
		.toBe('done');

	const done = (await (await request.get(`/api/jobs/${job.id}`)).json()) as JobSummary;
	const expected = vectors
		.filter((v) => v.seed === 42 && v.samples === 65536 && v.unit_index < 2)
		.reduce((sum, v) => sum + v.inside, 0);
	expect((done.result as unknown as PiResult).inside).toBe(expected);
	await expect(page.getByTestId('accepted')).not.toHaveText('0');

	// The dashboard reflects the finished job.
	const dashboard = await page.context().newPage();
	await dashboard.goto('/dashboard');
	await expect(dashboard.locator(`[data-job-id="${job.id}"]`)).toHaveAttribute(
		'data-status',
		'done'
	);
	await expect(dashboard.getByTestId('worker-count')).toHaveText('1');
});
