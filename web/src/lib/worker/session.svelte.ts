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

/**
 * Units held at once, computing or fetched in advance: enough that round trips overlap
 * the work, never more than the coordinator allows.
 */
const MIN_HELD = 2;
const MAX_HELD = 8;

type Assignment = Extract<CoordinatorMessage, { type: 'assign' }>;

/**
 * One contributor session: connects to the coordinator and computes units until stopped.
 *
 * Units are fetched in advance so the round trip to the coordinator (over a second
 * through a relay) overlaps the work instead of idling the device. How many depends on
 * the device: a fast GPU behind a slow link keeps several in flight, a slow CPU two.
 */
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
	/** Leases assigned whose result has not been sent yet (heartbeats keep them alive). */
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- bookkeeping, never rendered
	private held = new Set<string>();
	/** Assigned units waiting for the one computing to finish. */
	private queue: Assignment[] = [];
	private computing = false;
	/** Work requests sent and not yet answered. */
	private requested = 0;
	private requestSentAt: number[] = [];
	/** Moving averages of the round trip for a unit and of a unit's compute time. */
	private roundTripMs = 1000;
	private unitMs = 1000;
	/** Replies the coordinator owes us; it answers every message except heartbeats. */
	private owed = 0;
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

	/** Drops `ws` and reconnects with back-off. The coordinator returns our units to the queue. */
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
		// Units of a lost connection are the coordinator's again; a unit still computing
		// finishes, but its result has nowhere to go.
		this.held.clear();
		this.queue = [];
		this.requested = 0;
		this.requestSentAt = [];
		this.owed = 0;
	}

	private send(msg: WorkerMessage) {
		if (this.ws?.readyState === WebSocket.OPEN) this.ws.send(JSON.stringify(msg));
	}

	/** Sends a message the coordinator always answers, and reconnects if it doesn't. */
	private sendExpectingReply(msg: WorkerMessage) {
		const ws = this.ws;
		if (!ws) return;
		this.send(msg);
		this.owed++;
		if (this.owed === 1) this.armReplyTimeout(ws);
	}

	private armReplyTimeout(ws: WebSocket) {
		clearTimeout(this.replyTimeout);
		this.replyTimeout = setTimeout(() => this.lost(ws), REPLY_TIMEOUT_MS);
	}

	/** Units to hold so that the next ones arrive before the current one is done. */
	private wanted(): number {
		const inFlight = 1 + Math.ceil(this.roundTripMs / Math.max(this.unitMs, 1));
		return Math.min(MAX_HELD, Math.max(MIN_HELD, inFlight));
	}

	/** Asks for units until `wanted()` are held or requested. */
	private requestWork() {
		clearTimeout(this.retry);
		while (this.running && this.ws && this.held.size + this.requested < this.wanted()) {
			this.requested++;
			this.requestSentAt.push(performance.now());
			this.sendExpectingReply({ type: 'request_work' });
		}
	}

	/** A work request was answered (with a unit or with no work). */
	private answered() {
		this.requested = Math.max(0, this.requested - 1);
		const sent = this.requestSentAt.shift();
		if (sent !== undefined) {
			this.roundTripMs = 0.7 * this.roundTripMs + 0.3 * (performance.now() - sent);
		}
	}

	private onMessage(msg: CoordinatorMessage) {
		if (msg.type !== 'error') {
			this.owed = Math.max(0, this.owed - 1);
			clearTimeout(this.replyTimeout);
			if (this.owed > 0 && this.ws) this.armReplyTimeout(this.ws);
		}
		switch (msg.type) {
			case 'welcome':
				this.workerId = msg.worker_id;
				this.reconnectDelay = 1000;
				this.error = null;
				// Heartbeats keep the connection and every held lease alive.
				this.heartbeat = setInterval(
					() => this.send({ type: 'heartbeat', lease_ids: [...this.held] }),
					msg.heartbeat_interval_ms
				);
				this.requestWork();
				break;

			case 'assign':
				this.answered();
				this.held.add(msg.lease_id);
				this.queue.push(msg);
				void this.computeNext();
				// Fetch the next unit while this one computes.
				this.requestWork();
				break;

			case 'ack':
				if (msg.accepted) this.unitsAccepted++;
				else this.unitsRejected++;
				this.requestWork();
				break;

			case 'no_work':
				this.answered();
				if (!this.computing && this.held.size === 0) this.status = 'waiting';
				// Ask again later, once, unless something else asks first.
				if (this.requested === 0) {
					clearTimeout(this.retry);
					this.retry = setTimeout(() => this.requestWork(), msg.retry_after_ms);
				}
				break;

			case 'error':
				// The coordinator closes the connection next; onclose reconnects, except
				// for a device it has blocked, which would only be turned away again.
				this.error = msg.message;
				if (msg.message.startsWith('this device is blocked')) this.stop();
				break;
		}
	}

	/** Computes queued units one at a time. */
	private async computeNext() {
		if (this.computing) return;
		const ws = this.ws;
		for (let unit = this.queue.shift(); unit; unit = this.queue.shift()) {
			this.computing = true;
			this.status = 'computing';
			let reply: WorkerMessage;
			try {
				const { output, elapsed_ms } = await this.compute!.run(unit.template, unit.params);
				this.lastUnitMs = elapsed_ms;
				this.unitMs = 0.7 * this.unitMs + 0.3 * elapsed_ms;
				reply = { type: 'result', lease_id: unit.lease_id, output, elapsed_ms };
			} catch (e) {
				if (!this.running) break;
				const reason = e instanceof Error ? e.message : String(e);
				reply = { type: 'failed', lease_id: unit.lease_id, reason };
			}
			// A unit of a connection that has since been lost is not ours to answer.
			if (this.ws !== ws || !this.held.delete(unit.lease_id)) break;
			// The result goes out before any new request, so the coordinator never sees more
			// leases than this worker means to hold.
			this.sendExpectingReply(reply);
			this.requestWork();
		}
		this.computing = false;
		if (this.running && this.ws && this.queue.length === 0 && this.status === 'computing') {
			this.status = 'waiting';
		}
	}

	private fail(message: string) {
		this.error = message;
		this.stop();
	}
}
