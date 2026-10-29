import { ComputeClient } from './compute-client.ts';
import type { Backend } from '#lib/protocol/Backend.ts';
import type { CoordinatorMessage } from '#lib/protocol/CoordinatorMessage.ts';
import type { DeviceInfo } from '#lib/protocol/DeviceInfo.ts';
import type { WorkerMessage } from '#lib/protocol/WorkerMessage.ts';

export const CLIENT_VERSION = 'web-0.1.0';
const PROTOCOL_VERSION = 1;
const MAX_RECONNECT_DELAY_MS = 15_000;
// The coordinator answers `request_work` and `result` at once. No answer within this
// time means the connection is dead even if the socket still looks open: a phone
// that slept with the page frozen is dropped by the coordinator, but the browser
// may never see the close.
const REPLY_TIMEOUT_MS = 20_000;

export type SessionStatus =
	'stopped' | 'starting' | 'connecting' | 'computing' | 'waiting' | 'reconnecting' | 'unsupported';

const CLIENT_ID_KEY = 'swarm-client-id';

/**
 * This browser's id across visits: the coordinator keeps its reputation under it
 * (spec 0004). Without storage (private windows, blocked site data) every connection
 * is a new, untrusted client, which still works.
 */
function clientId(): string | undefined {
	try {
		let id = localStorage.getItem(CLIENT_ID_KEY);
		if (!id) {
			id = crypto.randomUUID();
			localStorage.setItem(CLIENT_ID_KEY, id);
		}
		return id;
	} catch {
		return undefined;
	}
}

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
	private replyTimeout: ReturnType<typeof setTimeout> | undefined;
	private reconnectDelay = 1000;

	/** `backend: 'wasm'` computes on the CPU even when there is a GPU. */
	constructor(
		private url: string,
		private backend?: Backend
	) {}

	async start() {
		if (this.running) return;
		this.running = true;
		this.error = null;
		this.status = 'starting';
		this.compute ??= new ComputeClient((reason) => this.fail(`GPU device lost: ${reason}`));
		try {
			this.device ??= await this.compute.init(this.backend);
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
			this.sendExpectingReply({
				type: 'hello',
				protocol_version: PROTOCOL_VERSION,
				client_version: CLIENT_VERSION,
				device: this.device!,
				client_id: clientId()
			});
		};
		ws.onmessage = (event) => this.onMessage(JSON.parse(event.data) as CoordinatorMessage);
		ws.onclose = () => this.lost(ws);
	}

	/** Drops `ws` and reconnects with back-off. The coordinator returns our unit to the queue. */
	private lost(ws: WebSocket) {
		if (this.ws !== ws) return;
		this.teardown();
		this.ws = null;
		// A half-open socket may take long to report its close; don't wait for it.
		ws.onmessage = ws.onclose = null;
		ws.close();
		if (!this.running) return;
		this.status = 'reconnecting';
		this.retry = setTimeout(() => this.running && this.connect(), this.reconnectDelay);
		this.reconnectDelay = Math.min(this.reconnectDelay * 2, MAX_RECONNECT_DELAY_MS);
	}

	private teardown() {
		clearInterval(this.heartbeat);
		clearTimeout(this.retry);
		clearTimeout(this.replyTimeout);
		this.lease = null;
	}

	private send(msg: WorkerMessage) {
		if (this.ws?.readyState === WebSocket.OPEN) this.ws.send(JSON.stringify(msg));
	}

	/** Sends a message the coordinator always answers, and reconnects if it doesn't. */
	private sendExpectingReply(msg: WorkerMessage) {
		const ws = this.ws;
		if (!ws) return;
		this.send(msg);
		clearTimeout(this.replyTimeout);
		this.replyTimeout = setTimeout(() => this.lost(ws), REPLY_TIMEOUT_MS);
	}

	private requestWork() {
		if (this.running) this.sendExpectingReply({ type: 'request_work' });
	}

	private async onMessage(msg: CoordinatorMessage) {
		clearTimeout(this.replyTimeout);
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
					this.sendExpectingReply({ type: 'result', lease_id: msg.lease_id, output, elapsed_ms });
				} catch (e) {
					if (!this.running) return;
					const reason = e instanceof Error ? e.message : String(e);
					this.sendExpectingReply({ type: 'failed', lease_id: msg.lease_id, reason });
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
