import { expect, test } from '@playwright/test';

// Pages and the API say they may not be framed or sniffed, and keep join and invite
// links to this site (spec 0016). Every page loads under its Content-Security-Policy
// without tripping it: its scripts, styles, fonts, WebAssembly and images are all its own.
test('pages carry their security headers and run under their content security policy', async ({
	page
}) => {
	await page.addInitScript(() => {
		const seen: string[] = [];
		Object.defineProperty(window, '__cspViolations', { value: seen });
		document.addEventListener('securitypolicyviolation', (e) =>
			seen.push(`${e.violatedDirective} ${e.blockedURI}`)
		);
	});

	for (const path of ['/', '/login', '/worker', '/selftest', '/dashboard']) {
		const res = await page.goto(path);
		const headers = res!.headers();
		const csp = headers['content-security-policy'];
		expect(csp, path).toContain("frame-ancestors 'none'");
		expect(csp, path).toContain("object-src 'none'");
		// Inline scripts run only with this response's nonce.
		expect(csp, path).toMatch(/script-src 'self' 'wasm-unsafe-eval' 'nonce-[^']+'/);
		expect(headers['x-frame-options'], path).toBe('DENY');
		expect(headers['x-content-type-options'], path).toBe('nosniff');
		expect(headers['referrer-policy'], path).toBe('same-origin');

		await page.waitForLoadState('load');
		// The page's own script ran: the shell's inline script marks the document.
		await expect(page.locator('html')).toHaveClass(/\bjs\b/);
		const violations = await page.evaluate(
			() => (window as unknown as { __cspViolations: string[] }).__cspViolations
		);
		expect(violations, path).toEqual([]);
	}

	const api = await page.request.get('/api/me');
	expect(api.headers()['content-security-policy']).toBe(
		"default-src 'none'; frame-ancestors 'none'"
	);
	expect(api.headers()['x-frame-options']).toBe('DENY');
});
