import { devices, expect, test, type Page } from '@playwright/test';
import { createNetwork, deleteNetwork } from './submit.ts';

// Spec 0017 in the browser: a network's device runs a whole language model (the catalog's
// tiny test model, random weights) through the real runtime, on the GPU or the CPU, and
// the console shows and downloads the answers.

const PROMPTS = ['Hello there', 'Name a colour', 'Count to three'];

/** Joins `page` to the network of `link` and waits until it asks for work. */
async function join(page: Page, link: string) {
	await page.goto(link);
	await page.getByRole('button', { name: 'Включи се' }).click();
	await expect(page.getByTestId('status')).toHaveAttribute('data-status', /waiting|computing/, {
		timeout: 30_000
	});
}

/** Starts a job of the test model from the console, trusting one answer per prompt. */
async function startJob(console: Page, networkUrl: string, prompts: string[]) {
	await console.goto(`${networkUrl}/jobs?test-models`);
	await console.getByTestId('kind-ai').click();
	await console
		.getByTestId('ai-model')
		.selectOption('hf-internal-testing/tiny-random-LlamaForCausalLM');
	await console.getByTestId('ai-tokens').selectOption('32');
	await console.getByTestId('ai-quorum').selectOption('1');
	await console.getByTestId('ai-prompts').fill(prompts.join('\n'));
	await console.getByTestId('start-job').click();
	await console
		.locator('[data-testid=job]:not([data-status=done])')
		.getByTestId('answers-link')
		.first()
		.click();
}

test('a language model answers on a device, from a checked cache the second time', async ({
	page,
	browser,
	browserName
}) => {
	test.setTimeout(240_000);
	await createNetwork(page, 'Есета');
	const networkUrl = page.url();
	const link = await page.getByTestId('join-link').inputValue();

	const context = await browser.newContext();
	const modelRequests: string[] = [];
	// The runtime never reaches another site, Hugging Face included.
	const offSite: string[] = [];
	context.on('request', (r) => {
		const url = new URL(r.url());
		if (url.pathname.startsWith('/api/models/')) modelRequests.push(url.pathname);
		if (url.host !== new URL(link).host) offSite.push(r.url());
	});
	const device = await context.newPage();
	await join(device, link);

	await startJob(page, networkUrl, PROMPTS);
	const answers = page.getByTestId('answers');
	await expect(answers).toHaveAttribute('data-status', 'done', { timeout: 150_000 });
	await expect(page.getByTestId('answer')).toHaveCount(3);
	await expect(page.locator('[data-testid=answer][data-answered=true]')).toHaveCount(3);
	// The model's five files came from this site, each once.
	expect(new Set(modelRequests).size).toBe(5);
	expect(modelRequests.length).toBe(5);

	// The answers download as a table, in prompt order.
	const download = page.waitForEvent('download');
	await page.getByTestId('download-csv').click();
	const csv = await (await download).createReadStream();
	let text = '';
	for await (const chunk of csv) text += chunk;
	const lines = text
		.replace(/^\u{FEFF}/u, '')
		.trim()
		.split('\r\n');
	expect(lines[0]).toBe('№,Въпрос,Отговор');
	expect(lines.slice(1).map((l) => l.split(',').slice(0, 2).join(','))).toEqual(
		PROMPTS.map((p, i) => `${i + 1},${p}`)
	);

	// The device kept a checked copy of each file.
	const cached = await device.evaluate(async () =>
		(await (await caches.open('swarm-models-v1')).keys()).map((r) => new URL(r.url).pathname)
	);
	expect(cached.sort()).toEqual([...new Set(modelRequests)].sort());

	// A fresh page (a new compute worker, nothing in memory) reads the model from that
	// copy: no request for it. Not in WebKit as Playwright opens it: a private session, in
	// which a new page's worker finds Cache Storage empty (checked 2026-11-21); a Safari
	// profile is to be checked on a real device.
	modelRequests.length = 0;
	const fresh = await context.newPage();
	await device.close();
	await join(fresh, link);
	await startJob(page, networkUrl, ['One more']);
	await expect(page.getByTestId('answers')).toHaveAttribute('data-status', 'done', {
		timeout: 100_000
	});
	if (browserName !== 'webkit') expect(modelRequests).toEqual([]);
	expect(offSite).toEqual([]);

	await context.close();
	await deleteNetwork(page, networkUrl);
});

test('a model file whose hash differs is refused and nothing is answered', async ({
	page,
	browser
}) => {
	test.setTimeout(180_000);
	await createNetwork(page, 'Подменен модел');
	const networkUrl = page.url();
	const link = await page.getByTestId('join-link').inputValue();

	const context = await browser.newContext();
	const device = await context.newPage();
	// Someone between the server and the device changes one byte of the tokenizer's
	// settings.
	await device.route('**/api/models/**/tokenizer_config.json', async (route) => {
		const res = await route.fetch();
		const body = (await res.text()).replace('"', ' "');
		await route.fulfill({ response: res, body });
	});
	await join(device, link);

	await startJob(page, networkUrl, ['Hello']);
	const answers = page.getByTestId('answers');
	// Each try is refused on the device; after the coordinator's limit the job fails.
	await expect(answers).toHaveAttribute('data-status', 'failed', { timeout: 150_000 });
	await expect(page.locator('[data-testid=answer][data-answered=true]')).toHaveCount(0);

	await context.close();
	await deleteNetwork(page, networkUrl);
});

test('a phone does not announce the GPU memory that language models are given by', async ({
	page,
	browser
}) => {
	test.setTimeout(120_000);
	await createNetwork(page, 'Телефон и компютър');
	const link = await page.getByTestId('join-link').inputValue();

	// The coordinator gives a language model's units only to a device that announces a GPU
	// buffer big enough (a real model needs 256 MB; the tiny test model needs none, so it
	// cannot show this). A computer announces its size; a phone, here Chromium saying it is
	// a Pixel with the same GPU, leaves it out.
	async function announces(context: import('@playwright/test').BrowserContext) {
		const hellos: string[] = [];
		const device = await context.newPage();
		device.on('websocket', (ws) =>
			ws.on('framesent', (f) => {
				if (typeof f.payload === 'string' && f.payload.includes('"device"')) hellos.push(f.payload);
			})
		);
		await join(device, link);
		await expect.poll(() => hellos.length).toBeGreaterThan(0);
		const hello = JSON.parse(hellos[0]) as {
			device: { backend: string; max_buffer_size?: number };
		};
		await context.close();
		return hello.device;
	}

	const computer = await announces(await browser.newContext());
	expect(computer.backend).toBe('webgpu');
	expect(computer.max_buffer_size).toBeGreaterThan(256 * 1024 * 1024);

	const phone = await announces(await browser.newContext({ ...devices['Pixel 7'] }));
	expect(phone.backend).toBe('webgpu');
	expect(phone.max_buffer_size).toBeUndefined();
});
