import { ComputeClient } from './compute-client.ts';
import { UnfitError } from '#lib/ai/check.ts';
import { cpuThreads } from '#lib/cpu/pool.ts';
import { announced } from '#lib/device-name.ts';
import { DEFAULT_POWER, restMs, type PauseReason } from './pacing.ts';
import type { Backend } from '#lib/protocol/Backend.ts';
import type { CoordinatorMessage } from '#lib/protocol/CoordinatorMessage.ts';
import type { DeviceControl } from '#lib/protocol/DeviceControl.ts';
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
// A pausing device waits this long for the answers it is owed before it disconnects.
const SETTLE_TIMEOUT_MS = 3_000;

export type SessionStatus =
	| 'stopped'
	| 'starting'
	| 'connecting'
	| 'computing'
	| 'waiting'
	| 'reconnecting'
	| 'paused'
	| 'held'
	| 'unsupported';

/** Errors after which reconnecting would only be turned away again. */
const FINAL_ERRORS = [
	'this device is blocked',
	'this device was removed',
	'this network does not exist'
];

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

// Templates whose CPU kernel uses a single thread: a CPU device computes one such unit
// per thread at once. The others use every thread, one unit at a time.
const ONE_THREAD = new Set(['nn']);
// Templates that run on the CPU on every device, one unit per thread: a network's own
// programs (spec 0012) and timetables (spec 0013).
const CPU_EVERYWHERE = new Set(['program', 'tt']);

type Assignment = Extract<CoordinatorMessage, { type: 'assign' }>;

/**
 * One contributor session: connects to the coordinator and computes units until stopped.
 *
 * Units are fetched in advance so the round trip to the coordinator (over a second
 * through a relay) overlaps the work instead of idling the device. How many depends on
 * the device: a fast GPU behind a slow link keeps several in flight, a slow CPU two.
 *
 * The device computes `power` % of the time and pauses for `setPause` (spec 0007): it
 * finishes the unit in hand, sends the result and disconnects, so units fetched in
 * advance go back to the queue at once.
 */
export class WorkerSession {
	status = $state<SessionStatus>('stopped');
	device = $state<DeviceInfo | null>(null);
	workerId = $state<string | null>(null);
	unitsAccepted = $state(0);
	unitsRejected = $state(0);
	lastUnitMs = $state<number | null>(null);
	error = $state<string | null>(null);
	/**
	 * Whether this device answered a language model's known question wrongly (spec 0017),
	 * so it takes none of that model's units.
	 */
	unfitForAi = $state(false);
	/** Share of the time spent computing, in percent. */
	power = $state<number>(DEFAULT_POWER);
	/** Why the device is paused, or null. */
	pausedFor = $state<PauseReason | null>(null);
	/**
	 * Whether this device also runs reviewed community programs on the public network
	 * (spec 0012). Read when connecting: a change applies from the next start.
	 */
	community = true;
	/** What the network's owner allows this device (spec 0010). */
	control = $state<DeviceControl>({ state: 'running' });
	/** The network's power limit, in percent. */
	maxPower = $state(100);

	private compute: ComputeClient | null = null;
	private ws: WebSocket | null = null;
	private running = false;
	/** Leases assigned whose result has not been sent yet (heartbeats keep them alive). */
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- bookkeeping, never rendered
	private held = new Set<string>();
	/** Assigned units waiting for a lane. */
	private queue: Assignment[] = [];
	/** Units computing at once: one, or one per CPU thread for ONE_THREAD templates. */
	private lanes = 0;
	/** Lanes the latest assigned unit allows; more units are fetched in advance for them. */
	private parallel = 1;
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
	/** Disconnect once every answer owed has arrived (pausing). */
	private disconnectWhenSettled = false;
	private settleTimer: ReturnType<typeof setTimeout> | undefined;
	/** Lanes resting between units, each with its wake-up timer. */
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- bookkeeping, never rendered
	private resting = new Map<() => void, ReturnType<typeof setTimeout>>();

	/**
	 * `backend: 'wasm'` computes on the CPU even when there is a GPU; `joinCode` joins
	 * that network instead of the public one.
	 */
	constructor(
		private url: string,
		private backend?: Backend,
		private joinCode?: string
	) {}

	/** The power the device actually uses: its owner's choice within the network's limit. */
	get effectivePower(): number {
		return Math.min(this.power, this.maxPower);
	}

	private get computing(): boolean {
		return this.lanes > 0;
	}

	/** Units of `unit`'s template this device computes at once. */
	private lanesFor(unit: Assignment): number {
		if (CPU_EVERYWHERE.has(unit.template.id)) return cpuThreads();
		const cpu = this.device?.backend === 'wasm' && ONE_THREAD.has(unit.template.id);
		return cpu ? cpuThreads() : 1;
	}

	/** Whether the network's owner lets this device compute now. */
	private get allowed(): boolean {
		return this.control.state === 'running';
	}

	async start() {
		if (this.running) return;
		this.running = true;
		this.error = null;
		this.status = 'starting';
		this.compute ??= new ComputeClient((reason) => this.fail(`GPU device lost: ${reason}`));
		try {
			this.device ??= announced(await this.compute.init(this.backend), navigator);
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
		this.interruptRest();
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

	/** Pauses for `reason`, or resumes with null. */
	setPause(reason: PauseReason | null) {
		const was = this.pausedFor;
		this.pausedFor = reason;
		if (!this.running || (reason === null) === (was === null)) return;
		if (reason) {
			this.interruptRest();
			clearTimeout(this.retry);
			if (!this.computing) this.settleThenDisconnect();
		} else if (this.ws) {
			// Resumed before the pause disconnected.
			this.disconnectWhenSettled = false;
			clearTimeout(this.settleTimer);
			this.requestWork();
			this.computeNext();
		} else {
			this.reconnectDelay = 1000;
			this.connect();
		}
	}

	private connect() {
		if (this.pausedFor) {
			this.status = 'paused';
			return;
		}
		this.status = 'connecting';
		const ws = new WebSocket(this.url);
		this.ws = ws;
		ws.onopen = () => {
			this.sendExpectingReply({
				type: 'hello',
				protocol_version: PROTOCOL_VERSION,
				client_version: CLIENT_VERSION,
				device: this.device!,
				client_id: clientId(),
				join_code: this.joinCode,
				community: this.community ? undefined : false
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
		if (this.pausedFor) {
			this.status = 'paused';
			return;
		}
		this.status = 'reconnecting';
		this.retry = setTimeout(() => this.running && this.connect(), this.reconnectDelay);
		this.reconnectDelay = Math.min(this.reconnectDelay * 2, MAX_RECONNECT_DELAY_MS);
	}

	private teardown() {
		clearInterval(this.heartbeat);
		clearTimeout(this.retry);
		clearTimeout(this.replyTimeout);
		clearTimeout(this.settleTimer);
		this.disconnectWhenSettled = false;
		// Units of a lost connection are the coordinator's again; a unit still computing
		// finishes, but its result has nowhere to go.
		this.held.clear();
		this.queue = [];
		this.requested = 0;
		this.requestSentAt = [];
		this.owed = 0;
	}

	/** Closes the connection on purpose (pausing): no reconnect follows. */
	private disconnect() {
		const ws = this.ws;
		this.teardown();
		this.ws = null;
		if (ws) {
			ws.onmessage = ws.onclose = null;
			ws.close();
		}
		if (this.running) this.status = 'paused';
	}

	/** Disconnects once the coordinator has answered everything it owes. */
	private settleThenDisconnect() {
		if (!this.ws || this.owed === 0) {
			this.disconnect();
			return;
		}
		this.disconnectWhenSettled = true;
		clearTimeout(this.settleTimer);
		this.settleTimer = setTimeout(() => this.disconnect(), SETTLE_TIMEOUT_MS);
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
		// Resting stretches each unit to `unitMs · 100 / power`.
		const paced = (this.unitMs * 100) / this.effectivePower;
		const inFlight = 1 + Math.ceil(this.roundTripMs / Math.max(paced, 1));
		return Math.min(MAX_HELD, Math.max(MIN_HELD, inFlight) + this.parallel - 1);
	}

	/** Asks for units until `wanted()` are held or requested. */
	private requestWork() {
		clearTimeout(this.retry);
		while (
			this.running &&
			this.ws &&
			!this.pausedFor &&
			this.allowed &&
			this.held.size + this.requested < this.wanted()
		) {
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
		// Errors and control messages come unasked; everything else answers a request.
		if (msg.type !== 'error' && msg.type !== 'control') {
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
				this.parallel = this.lanesFor(msg);
				this.computeNext();
				// Fetch the next unit while this one computes.
				this.requestWork();
				break;

			case 'ack':
				if (msg.accepted) this.unitsAccepted++;
				else this.unitsRejected++;
				this.requestWork();
				break;

			case 'control':
				this.control = msg.control;
				this.maxPower = msg.max_power;
				if (this.allowed) {
					if (this.status === 'held') this.status = 'waiting';
					this.requestWork();
					this.computeNext();
				} else if (!this.computing && this.held.size === 0) {
					// Units already in hand are finished; no new ones are asked for.
					this.status = 'held';
				}
				break;

			case 'no_work':
				this.answered();
				if (!this.computing && this.held.size === 0) {
					this.status = this.allowed ? 'waiting' : 'held';
				}
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
				if (FINAL_ERRORS.some((e) => msg.message.startsWith(e))) this.stop();
				break;
		}
		if (this.disconnectWhenSettled && this.owed === 0) this.disconnect();
	}

	/** Opens lanes for the queued units, as many as the next unit allows. */
	private computeNext() {
		while (this.running && this.queue.length > 0 && this.lanes < this.lanesFor(this.queue[0])) {
			this.lanes++;
			void this.lane();
		}
	}

	/** Computes queued units one after another, resting between them at reduced power. */
	private async lane() {
		for (let unit = this.nextUnit(); unit; unit = this.nextUnit()) {
			// The queue only holds units of the current connection, but the connection may
			// be lost and replaced while this one computes.
			const ws = this.ws;
			this.status = 'computing';
			let reply: WorkerMessage;
			let elapsed = 0;
			try {
				const { output, elapsed_ms } = await this.compute!.run(unit.template, unit.params);
				elapsed = elapsed_ms;
				this.lastUnitMs = elapsed_ms;
				this.unitMs = 0.7 * this.unitMs + 0.3 * elapsed_ms;
				reply = { type: 'result', lease_id: unit.lease_id, output, elapsed_ms };
			} catch (e) {
				if (!this.running) break;
				const reason = e instanceof Error ? e.message : String(e);
				// A device that computes a model wrongly is given none of the job's units again.
				const unfit = e instanceof UnfitError;
				if (unfit) this.unfitForAi = true;
				reply = { type: 'failed', lease_id: unit.lease_id, reason, ...(unfit ? { unfit } : {}) };
			}
			// A unit of a connection that has since been lost is not ours to answer; units
			// of the new connection may already be queued behind it.
			if (this.ws !== ws || !this.held.delete(unit.lease_id)) continue;
			// The result goes out before any new request, so the coordinator never sees more
			// leases than this worker means to hold.
			this.sendExpectingReply(reply);
			this.requestWork();
			const rest = restMs(elapsed, this.effectivePower);
			if (rest > 0 && this.running && !this.pausedFor) await this.rest(rest);
		}
		this.lanes--;
		if (!this.running || this.lanes > 0) return;
		if (this.pausedFor) {
			this.settleThenDisconnect();
		} else if (this.ws && this.queue.length === 0 && this.status === 'computing') {
			this.status = this.allowed ? 'waiting' : 'held';
		}
	}

	/**
	 * The next unit for a lane; none while paused, or when the next unit allows fewer
	 * lanes than are open (this lane closes; the others carry on).
	 */
	private nextUnit(): Assignment | undefined {
		const next = this.queue[0];
		if (this.pausedFor || !next || this.lanes > this.lanesFor(next)) return undefined;
		return this.queue.shift();
	}

	/** Waits `ms`, or until a pause or stop interrupts. */
	private rest(ms: number): Promise<void> {
		return new Promise((resolve) => {
			const wake = () => {
				this.resting.delete(wake);
				resolve();
			};
			this.resting.set(
				wake,
				setTimeout(() => wake(), ms)
			);
		});
	}

	/** Ends every lane's rest at once. */
	private interruptRest() {
		for (const [wake, timer] of this.resting) {
			clearTimeout(timer);
			wake();
		}
	}

	private fail(message: string) {
		this.error = message;
		this.stop();
	}
}
