import { expect, test } from '@playwright/test';
import type { JobSummary } from '../src/lib/protocol/JobSummary.ts';
import type { PtResult } from '../src/lib/protocol/PtResult.ts';

test('a browser worker renders an image that the render page shows', async ({
	page,
	request
}, testInfo) => {
	await page.goto('/worker');
	await page.getByRole('button', { name: 'Включи се' }).click();
	await expect(page.getByTestId('device')).toHaveAttribute(
		'data-backend',
		testInfo.project.name === 'cpu' ? 'wasm' : 'webgpu'
	);

	// 4 × 3 tiles, two passes.
	const res = await request.post('/api/jobs', {
		data: {
			template: 'pt',
			params: { width: 64, height: 36, tile: 16, spp: 2, passes: 2, seed: 5 }
		}
	});
	expect(res.status()).toBe(201);
	const job = (await res.json()) as JobSummary;
	expect(job.units_total).toBe(24);

	const render = await page.context().newPage();
	await render.goto(`/render/${job.id}`);
	await expect(render.getByTestId('render-status')).toHaveAttribute('data-status', 'done', {
		timeout: 45_000
	});
	await expect(render.getByTestId('render-image')).toBeVisible();

	const done = (await (await request.get(`/api/jobs/${job.id}`)).json()) as JobSummary;
	const result = done.result as unknown as PtResult;
	expect([result.width, result.height, result.samples_per_pixel]).toEqual([64, 36, 4]);
	expect(result.mean.every((m) => m > 0)).toBe(true);

	const image = await request.get(`/api/jobs/${job.id}/image.png`);
	expect(image.headers()['content-type']).toBe('image/png');
	// PNG signature, then the IHDR width and height.
	const png = await image.body();
	expect(png.subarray(1, 4).toString()).toBe('PNG');
	expect([png.readUInt32BE(16), png.readUInt32BE(20)]).toEqual([64, 36]);
});
