import type { WorkerSummary } from '#lib/protocol/WorkerSummary.ts';

/**
 * The home page's live honeycomb (docs/design/design-system.md, "The honeycomb is live"): one
 * cell per connected device, from the live dashboard stream. A cell appears when its
 * device joins, lights when the device returns finished work, dims once it has been
 * idle, fades when the device leaves and turns red when it is caught cheating.
 *
 * This module is the model and the geometry only; LiveHive.svelte draws it. Times are
 * wall-clock milliseconds (Date.now()), like the dashboard state's.
 */

/** A device that has finished nothing for this long is idle. */
export const IDLE_MS = 10_000;
export const APPEAR_MS = 640;
export const FLASH_MS = 560;
export const LEAVE_MS = 420;
/** How long dimming to idle (or waking from it) takes. */
export const DIM_MS = 600;

export type Kind = 'gpu' | 'cpu' | 'server';

export interface Point {
	x: number;
	y: number;
}

export interface Layout {
	/** Cell radius (centre to corner), in CSS pixels. */
	r: number;
	/** Cell centres, nearest the focal point first. */
	slots: Point[];
}

/**
 * Pointy-top hexagons tiling `w × h` (with a cell of overhang so the edges are full),
 * at the largest radius up to `base` that leaves room for `needed` cells. Slots are
 * ordered by distance from the focal point, so the swarm grows from the middle.
 */
export function layout(w: number, h: number, needed: number, base: number, focus: Point): Layout {
	let r = base;
	for (;;) {
		const slots = grid(w, h, r);
		if (slots.length >= needed || r <= 3) {
			const d = (p: Point) => (p.x - focus.x) ** 2 + (p.y - focus.y) ** 2;
			const angle = (p: Point) => Math.atan2(p.y - focus.y, p.x - focus.x);
			slots.sort((a, b) => d(a) - d(b) || angle(a) - angle(b));
			return { r, slots };
		}
		r *= 0.88;
	}
}

function grid(w: number, h: number, r: number): Point[] {
	const dx = Math.sqrt(3) * r;
	const dy = 1.5 * r;
	const cols = Math.ceil(w / dx) + 1;
	const rows = Math.ceil(h / dy) + 1;
	const slots: Point[] = [];
	for (let row = 0; row < rows; row++) {
		for (let col = 0; col < cols; col++) {
			slots.push({ x: dx * (col + (row % 2) / 2), y: dy * row });
		}
	}
	return slots;
}

export interface Cell {
	id: string;
	kind: Kind;
	slot: number;
	connectedAt: number;
	unitsDone: number;
	joinedAt: number;
	/** When the device last returned work, or null if it has not since the page opened. */
	workedAt: number | null;
	/** When its connection closed; the cell fades, then frees its slot. */
	leftAt: number | null;
	caught: boolean;
}

/** One cell as drawn at a moment: 0–1 values, ready for the canvas. */
export interface CellFrame {
	slot: number;
	kind: Kind;
	caught: boolean;
	/** Overall visibility: appearing, leaving. */
	alpha: number;
	/** Size, 0.7–1: cells grow in as they appear and shrink a little as they leave. */
	scale: number;
	/** 1 while working, falling towards IDLE_ALPHA when idle. */
	activity: number;
	/** The moment of a returned result, fading. */
	flash: number;
}

const ease = (t: number) => 1 - (1 - Math.min(1, Math.max(0, t))) ** 3;

export const kindOf = (w: WorkerSummary): Kind =>
	w.device.backend === 'webgpu' ? 'gpu' : w.device.backend === 'wasm' ? 'cpu' : 'server';

export class HiveModel {
	readonly cells = new Map<string, Cell>();
	private capacity = 0;
	private seeded = false;

	/** `reduced`: prefers-reduced-motion. States are drawn at once, with no flashes. */
	constructor(public reduced = false) {}

	/** Cells that hold a slot, leaving ones included. */
	get size() {
		return this.cells.size;
	}

	/**
	 * Takes the devices now connected and the ids caught cheating. Returns true if
	 * anything changed. The first call only seeds: devices already present appear
	 * without flashing, as if they had been there all along.
	 */
	update(workers: WorkerSummary[], caught: ReadonlySet<string>, now: number): boolean {
		let changed = false;
		const present = new Set<string>();
		for (const w of workers) {
			present.add(w.id);
			const cell = this.cells.get(w.id);
			if (!cell) {
				this.cells.set(w.id, {
					id: w.id,
					kind: kindOf(w),
					slot: -1,
					connectedAt: w.connected_at,
					unitsDone: w.units_done,
					joinedAt: this.seeded ? now : now - APPEAR_MS,
					workedAt: null,
					leftAt: null,
					caught: caught.has(w.id)
				});
				changed = true;
				continue;
			}
			if (cell.leftAt !== null) {
				// Back before its cell faded: the same cell, as it was.
				cell.leftAt = null;
				changed = true;
			}
			if (w.units_done > cell.unitsDone) {
				cell.unitsDone = w.units_done;
				cell.workedAt = now;
				changed = true;
			}
			const isCaught = caught.has(w.id);
			if (isCaught !== cell.caught) {
				cell.caught = isCaught;
				changed = true;
			}
		}
		for (const cell of this.cells.values()) {
			if (present.has(cell.id) || cell.leftAt !== null) continue;
			cell.leftAt = now;
			changed = true;
		}
		this.seeded = true;
		this.prune(now);
		this.assign();
		return changed;
	}

	/** The field has `capacity` slots now (after a resize, say): every device is placed
	 * again, longest connected nearest the middle. */
	setCapacity(capacity: number) {
		if (capacity === this.capacity) return;
		this.capacity = capacity;
		for (const cell of this.cells.values()) cell.slot = -1;
		this.assign();
	}

	/** Devices fully faded out free their slots. */
	prune(now: number) {
		for (const [id, cell] of this.cells) {
			if (cell.leftAt !== null && (this.reduced || now - cell.leftAt >= LEAVE_MS)) {
				this.cells.delete(id);
			}
		}
	}

	/** Gives every unplaced cell the free slot nearest the middle. Existing cells keep
	 * theirs, so nothing moves when another device joins or leaves. */
	private assign() {
		const used = new Set<number>();
		const unplaced: Cell[] = [];
		for (const cell of this.cells.values()) {
			if (cell.slot >= 0 && cell.slot < this.capacity) used.add(cell.slot);
			else unplaced.push(cell);
		}
		unplaced.sort((a, b) => a.connectedAt - b.connectedAt || a.id.localeCompare(b.id));
		let free = 0;
		for (const cell of unplaced) {
			while (used.has(free)) free++;
			// More devices than slots: the rest wait for room (the caller grows the field).
			cell.slot = free < this.capacity ? free : -1;
			used.add(free);
		}
	}

	/** What to draw at `now`. */
	frame(now: number): CellFrame[] {
		const frames: CellFrame[] = [];
		for (const cell of this.cells.values()) {
			if (cell.slot < 0) continue;
			const appear = this.reduced ? 1 : ease((now - cell.joinedAt) / APPEAR_MS);
			const leave =
				cell.leftAt === null ? 0 : this.reduced ? 1 : ease((now - cell.leftAt) / LEAVE_MS);
			frames.push({
				slot: cell.slot,
				kind: cell.kind,
				caught: cell.caught,
				alpha: appear * (1 - leave),
				scale: (0.7 + 0.3 * appear) * (1 - 0.15 * leave),
				activity: this.activity(cell, now),
				flash:
					this.reduced || cell.workedAt === null ? 0 : 1 - ease((now - cell.workedAt) / FLASH_MS)
			});
		}
		return frames;
	}

	/** 1 while the device works, 0 once idle; eases between the two. */
	private activity(cell: Cell, now: number): number {
		if (cell.workedAt === null) return 0;
		const since = now - cell.workedAt;
		if (since < IDLE_MS) {
			// Waking from idle is part of the flash; nothing to ease here.
			return 1;
		}
		return this.reduced ? 0 : 1 - ease((since - IDLE_MS) / DIM_MS);
	}

	/** True while some cell is mid-transition, so the canvas must keep drawing. */
	animating(now: number): boolean {
		if (this.reduced) return false;
		for (const cell of this.cells.values()) {
			if (cell.slot < 0) continue;
			if (now - cell.joinedAt < APPEAR_MS) return true;
			if (cell.leftAt !== null) return true;
			if (cell.workedAt !== null) {
				const since = now - cell.workedAt;
				if (since < FLASH_MS) return true;
				if (since >= IDLE_MS && since < IDLE_MS + DIM_MS) return true;
			}
		}
		return false;
	}

	/** When the next transition starts with no new event (a device going idle), or null. */
	nextChange(now: number): number | null {
		let next: number | null = null;
		for (const cell of this.cells.values()) {
			if (cell.workedAt === null) continue;
			const idleAt = cell.workedAt + IDLE_MS;
			if (idleAt > now && (next === null || idleAt < next)) next = idleAt;
		}
		return next;
	}
}
