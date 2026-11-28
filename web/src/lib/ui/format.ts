/**
 * A number for people, written the Bulgarian way: a decimal comma and a space between
 * thousands (`5,6`, `12 400`). One decimal below ten, none above, as the live counters
 * have always shown them.
 */
export function rate(n: number): string {
	return n.toLocaleString('bg-BG', { maximumFractionDigits: n < 10 ? 1 : 0 });
}

/**
 * A running total for people: exact with spaces up to 99 999 (`12 400`), then short
 * (`1,2 млн.`), so a counter never wraps on a phone.
 */
export function total(n: number): string {
	if (n < 100_000) return n.toLocaleString('bg-BG');
	return n.toLocaleString('bg-BG', { notation: 'compact', maximumFractionDigits: 1 });
}
