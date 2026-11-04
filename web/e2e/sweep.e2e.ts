import { expect, test } from '@playwright/test';
import { signIn } from './submit.ts';

// Spec 0011 in the browser: a sweep of small neural networks trains on a device of the
// network, through the real kernels (GPU or CPU), and the console ranks it.
test('a neural network sweep trains on a device and is ranked', async ({ page, browser }) => {
	test.setTimeout(150_000);
	await signIn(page);
	await page.getByTestId('new-network-name').fill('Кръжок по ИИ');
	await page.getByRole('button', { name: 'Създай' }).click();
	await expect(page.getByTestId('network-title')).toHaveText('Кръжок по ИИ');
	const networkUrl = page.url();
	const link = await page.getByTestId('join-link').inputValue();

	const context = await browser.newContext();
	const device = await context.newPage();
	await device.goto(link);
	await device.getByRole('button', { name: 'Включи се' }).click();
	await expect(device.getByTestId('status')).toHaveAttribute('data-status', /waiting|computing/, {
		timeout: 30_000
	});

	// A small sweep: two layer sizes × two learning rates, short training.
	await page.goto(`${networkUrl}/jobs`);
	await page.getByTestId('kind-nn').click();
	await page.getByRole('button', { name: 'Бързо сравнение' }).click();
	await expect(page.getByTestId('sweep-size')).toContainText('6 настройки');
	await page.getByText('Подробни настройки').click();
	await page.getByLabel('Неврони в слой').fill('8, 16');
	await page.getByLabel('Скорост на учене').fill('0.05 0.3');
	await page.getByLabel('Партида').fill('16');
	await page.getByLabel('Стъпки').fill('40');
	await expect(page.getByTestId('sweep-size')).toContainText('4 настройки');
	await page.getByTestId('start-job').click();

	await page.getByTestId('sweep-link').first().click();
	const sweep = page.getByTestId('sweep');
	await expect(sweep).toHaveAttribute('data-status', 'done', { timeout: 100_000 });
	await expect(page.getByTestId('sweep-row')).toHaveCount(4);
	await expect(page.getByTestId('heat-cell')).toHaveCount(4);
	await expect(page.getByTestId('best-accuracy')).toHaveText(/^\d+\.\d %$/);

	// The best network can be downloaded; its accuracy is the one on the board.
	const href = await page.getByTestId('download-model').getAttribute('href');
	const model = await (await page.request.get(href!)).json();
	const best = Number((await page.getByTestId('best-accuracy').textContent())!.replace(' %', ''));
	expect(model.network).toHaveLength(2);
	expect(Math.abs(model.accuracy * 100 - best)).toBeLessThan(0.051);

	// The same network reads a digit drawn on the page, as the pen moves.
	const pad = page.getByTestId('digit-pad');
	await expect(pad).toHaveAttribute('data-ready', 'true', { timeout: 30_000 });
	await pad.scrollIntoViewIfNeeded();
	const box = (await pad.locator('canvas.pad').boundingBox())!;
	await page.mouse.move(box.x + box.width * 0.5, box.y + box.height * 0.15);
	await page.mouse.down();
	await page.mouse.move(box.x + box.width * 0.5, box.y + box.height * 0.85, { steps: 12 });
	await page.mouse.up();
	await expect(page.getByTestId('digit-guess')).toHaveText(/^\d$/);
	await page.getByRole('button', { name: 'Изчисти' }).click();
	await expect(page.getByTestId('digit-guess')).toHaveText('–');

	// Clean up: later tests start with an empty network list.
	await page.goto(`${networkUrl}/rules`);
	await page.getByTestId('delete-network').click();
	await page.getByTestId('delete-network').click();
	await page.waitForURL(/\/app$/);
	await context.close();
});
