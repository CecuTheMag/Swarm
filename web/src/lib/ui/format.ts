/**
 * A number for people, written the Bulgarian way: a decimal comma and a space between
 * thousands (`5,6`, `12 400`). One decimal below ten, none above, as the live counters
 * have always shown them.
 */
export function rate(n: number): string {
	return n.toLocaleString('bg-BG', { maximumFractionDigits: n < 10 ? 1 : 0 });
}
