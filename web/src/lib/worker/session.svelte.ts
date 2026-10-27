import { ComputeClient } from './compute-client.ts';
import type { CoordinatorMessage } from '#lib/protocol/CoordinatorMessage.ts';
import type { DeviceInfo } from '#lib/protocol/DeviceInfo.ts';
import type { WorkerMessage } from '#lib/protocol/WorkerMessage.ts';

export const CLIENT_VERSION = 'web-0.1.0';
const PROTOCOL_VERSION = 1;
const MAX_RECONNECT_DELAY_MS = 15_000;

export type SessionStatus =
	'stopped' | 'starting' | 'connecting' | 'computing' | 'waiting' | 'reconnecting' | 'unsupported';

/** One contributor session: connects to the coordinator and computes units until stopped. */
export class WorkerSession {
	status = $state<SessionStatus>('stopped');
	device = $state<DeviceInfo | null>(null);
	workerId = $state<string | null>(null);
	unitsAccepted = $state(0);
	unitsRejected = $state(0);
	lastUnitMs = $state<number | null>(null);
	error = $state<string | null>(null);

	private compute: ComputeClient | null = null;
	private ws: WebSocket | null = null;
	private running = false;
	private lease: string | null = null;
	private heartbeat: ReturnType<typeof setInterval> | undefined;
	private retry: ReturnType<typeof setTimeout> | undefined;
	private reconnectDelay = 1000;

	constructor(private url: string) {}

	async start() {
		if (this.running) return;
		this.running = true;
		this.error = null;
		this.status = 'starting';
		this.compute ??= new ComputeClient((reason) => this.fail(`GPU device lost: ${reason}`));
		try {
			this.device ??= await this.compute.init();
		} catch (e) {
			this.running = false;
			this.status = 'unsupported';
			this.error = e instanceof Error ? e.message : String(e);
			return;
		}
		if (this.running) this.connect();
	}

	stop() {
		this.running = false;
		this.compute?.abort();
		this.teardown();
		this.ws?.close();
		this.ws = null;
		this.status = 'stopped';
	}

	destroy() {
		this.stop();
		this.compute?.terminate();
		this.compute = null;
	}

	private connect() {
		this.status = 'connecting';
		const ws = new WebSocket(this.url);
		this.ws = ws;
		ws.onopen = () => {
			this.send({
				type: 'hello',
				protocol_version: PROTOCOL_VERSION,
				client_version: CLIENT_VERSION,
				device: this.device!
			});
		};
		ws.onmessage = (event) => this.onMessage(JSON.parse(event.data) as CoordinatorMessage);
		ws.onclose = () => {
			if (this.ws !== ws) return;
			this.teardown();
			this.ws = null;
			if (!this.running) return;
			// The coordinator returns our unit to the queue; reconnect with back-off.
			this.status = 'reconnecting';
			this.retry = setTimeout(() => this.running && this.connect(), this.reconnectDelay);
			this.reconnectDelay = Math.min(this.reconnectDelay * 2, MAX_RECONNECT_DELAY_MS);
		};
	}

	private teardown() {
		clearInterval(this.heartbeat);
		clearTimeout(this.retry);
		this.lease = null;
	}

	private send(msg: WorkerMessage) {
		if (this.ws?.readyState === WebSocket.OPEN) this.ws.send(JSON.stringify(msg));
	}

	private requestWork() {
		if (this.running) this.send({ type: 'request_work' });
	}

	private async onMessage(msg: CoordinatorMessage) {
		switch (msg.type) {
			case 'welcome':
				this.workerId = msg.worker_id;
				this.reconnectDelay = 1000;
				this.error = null;
				// Heartbeats keep the connection and any held lease alive.
				this.heartbeat = setInterval(
					() => this.send({ type: 'heartbeat', lease_ids: this.lease ? [this.lease] : [] }),
					msg.heartbeat_interval_ms
				);
				this.requestWork();
				break;

			case 'assign': {
				this.lease = msg.lease_id;
				this.status = 'computing';
				try {
					const { output, elapsed_ms } = await this.compute!.run(msg.template, msg.params);
					this.lastUnitMs = elapsed_ms;
					this.send({ type: 'result', lease_id: msg.lease_id, output, elapsed_ms });
				} catch (e) {
					if (!this.running) return;
					const reason = e instanceof Error ? e.message : String(e);
					this.send({ type: 'failed', lease_id: msg.lease_id, reason });
				}
				break;
			}

			case 'ack':
				if (msg.lease_id === this.lease) this.lease = null;
				if (msg.accepted) this.unitsAccepted++;
				else this.unitsRejected++;
				this.requestWork();
				break;

			case 'no_work':
				this.status = 'waiting';
				this.retry = setTimeout(() => this.requestWork(), msg.retry_after_ms);
				break;

			case 'error':
				// The coordinator closes the connection next; onclose reconnects.
				this.error = msg.message;
				break;
		}
	}

	private fail(message: string) {
		this.error = message;
		this.stop();
	}
}
