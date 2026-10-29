import { apply, emptyState } from './state.ts';
import { wsUrl } from '#lib/net.ts';
import type { DashboardEvent } from '#lib/protocol/DashboardEvent.ts';

/**
 * The coordinator's live event stream as reactive state: connects to /ws/dashboard,
 * reconnects when the connection drops, and ticks `now` once a second for rates.
 */
export class LiveDashboard {
	state = $state(emptyState());
	connected = $state(false);
	now = $state(Date.now());

	private ws: WebSocket | null = null;
	private closed = false;
	private retry: ReturnType<typeof setTimeout> | undefined;
	private tick: ReturnType<typeof setInterval> | undefined;

	/** `onEvent` sees every event after it has been applied. */
	constructor(private onEvent?: (event: DashboardEvent) => void) {}

	start() {
		this.closed = false;
		this.connect();
		this.tick = setInterval(() => (this.now = Date.now()), 1000);
	}

	stop() {
		this.closed = true;
		clearTimeout(this.retry);
		clearInterval(this.tick);
		this.ws?.close();
	}

	private connect() {
		const socket = new WebSocket(wsUrl('/ws/dashboard'));
		this.ws = socket;
		socket.onopen = () => (this.connected = true);
		socket.onmessage = (e) => {
			const event = JSON.parse(e.data) as DashboardEvent;
			this.state = apply(this.state, event, Date.now());
			this.onEvent?.(event);
		};
		socket.onclose = () => {
			this.connected = false;
			if (!this.closed) this.retry = setTimeout(() => this.connect(), 2000);
		};
	}
}
