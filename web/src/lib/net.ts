/** WebSocket URL on the page's own origin (the coordinator is served same-origin). */
export function wsUrl(path: string): string {
	const scheme = location.protocol === 'https:' ? 'wss' : 'ws';
	return `${scheme}://${location.host}${path}`;
}
