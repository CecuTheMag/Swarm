import { describe, expect, it } from 'vitest';
import { APPEAR_MS, FLASH_MS, HiveModel, IDLE_MS, LEAVE_MS, DIM_MS, layout } from './hive-field.ts';
import type { WorkerSummary } from '#lib/protocol/WorkerSummary.ts';

const worker = (
	id: string,
	units_done = 0,
	backend: 'webgpu' | 'wasm' | 'native' = 'webgpu',
	connected_at = 0
): WorkerSummary => ({
	id,
	units_done,
	connected_at,
	device: { backend, vendor: 'v', architecture: 'a' }
});

const none = new Set<string>();

function model(reduced = false) {
	const m = new HiveModel(reduced);
	m.setCapacity(10);
	return m;
}

describe('layout', () => {
	it('orders slots from the focal point outwards', () => {
		const { slots } = layout(400, 300, 5, 24, { x: 200, y: 150 });
		const d = slots.map((p) => Math.hypot(p.x - 200, p.y - 150));
		expect(d).toEqual([...d].sort((a, b) => a - b));
	});

	it('shrinks the cells until every device fits', () => {
		const small = layout(400, 300, 10, 24, { x: 200, y: 150 });
		const many = layout(400, 300, 1000, 24, { x: 200, y: 150 });
		expect(small.r).toBe(24);
		expect(many.r).toBeLessThan(24);
		expect(many.slots.length).toBeGreaterThanOrEqual(1000);
	});
});

describe('HiveModel', () => {
	it('shows devices present at the start without an entrance or a flash', () => {
		const m = model();
		m.update([worker('a', 5)], none, 1000);
		const [cell] = m.frame(1000);
		expect(cell.alpha).toBe(1);
		expect(cell.flash).toBe(0);
		expect(m.animating(1000)).toBe(false);
	});

	it('grows a joining device in, without passing full size', () => {
		const m = model();
		m.update([], none, 0);
		m.update([worker('a')], none, 1000);
		const sizes = [0, 100, 300, APPEAR_MS].map((t) => m.frame(1000 + t)[0].scale);
		expect(sizes[0]).toBeCloseTo(0.7);
		for (let i = 1; i < sizes.length; i++) expect(sizes[i]).toBeGreaterThanOrEqual(sizes[i - 1]);
		expect(Math.max(...sizes)).toBeLessThanOrEqual(1);
		expect(m.frame(1000 + APPEAR_MS)[0].alpha).toBe(1);
	});

	it('lights a device when it returns work, and dims it once idle', () => {
		const m = model();
		m.update([worker('a', 0)], none, 0);
		expect(m.frame(0)[0].activity).toBe(0);

		m.update([worker('a', 1)], none, 1000);
		expect(m.frame(1000)[0].flash).toBe(1);
		expect(m.frame(1000 + FLASH_MS)[0].flash).toBe(0);
		expect(m.frame(1000 + IDLE_MS - 1)[0].activity).toBe(1);
		expect(m.animating(1000 + FLASH_MS + 10)).toBe(false);
		expect(m.nextChange(1000 + FLASH_MS + 10)).toBe(1000 + IDLE_MS);
		expect(m.animating(1000 + IDLE_MS + 10)).toBe(true);
		expect(m.frame(1000 + IDLE_MS + DIM_MS)[0].activity).toBe(0);
	});

	it('fades a leaving device, then frees its slot for the next', () => {
		const m = model();
		m.update([worker('a', 0, 'webgpu', 1), worker('b', 0, 'webgpu', 2)], none, 0);
		const slotA = m.frame(0).find((c) => c.slot === 0);
		expect(slotA).toBeDefined();
		m.update([worker('b', 0, 'webgpu', 2)], none, 100);
		expect(m.size).toBe(2);
		expect(m.frame(100 + LEAVE_MS / 2).find((c) => c.slot === 0)!.alpha).toBeLessThan(1);
		m.update([worker('b', 0, 'webgpu', 2)], none, 100 + LEAVE_MS);
		expect(m.size).toBe(1);
		m.update([worker('b', 0, 'webgpu', 2), worker('c', 0, 'wasm', 3)], none, 1000);
		const c = m.frame(1000).find((f) => f.kind === 'cpu')!;
		expect(c.slot).toBe(0);
	});

	it('keeps every other device in its place when one joins or leaves', () => {
		const m = model();
		m.update([worker('a', 0, 'webgpu', 1), worker('b', 0, 'webgpu', 2)], none, 0);
		const before = new Map([...m.cells.values()].map((c) => [c.id, c.slot]));
		m.update([worker('b', 0, 'webgpu', 2), worker('z', 0, 'webgpu', 0)], none, 10);
		expect(m.cells.get('b')!.slot).toBe(before.get('b'));
	});

	it('marks a device caught cheating', () => {
		const m = model();
		m.update([worker('a')], none, 0);
		m.update([worker('a')], new Set(['a']), 10);
		expect(m.frame(10)[0].caught).toBe(true);
	});

	it('with reduced motion, draws each state at once', () => {
		const m = model(true);
		m.update([], none, 0);
		m.update([worker('a', 0)], none, 10);
		expect(m.frame(10)[0]).toMatchObject({ alpha: 1, scale: 1, flash: 0 });
		m.update([worker('a', 1)], none, 20);
		expect(m.frame(20)[0]).toMatchObject({ flash: 0, activity: 1 });
		expect(m.animating(20)).toBe(false);
		expect(m.frame(20 + IDLE_MS)[0].activity).toBe(0);
		m.update([], none, 30);
		expect(m.size).toBe(0);
	});

	it('places only as many devices as there are slots', () => {
		const m = new HiveModel();
		m.setCapacity(2);
		m.update(
			['a', 'b', 'c'].map((id, i) => worker(id, 0, 'webgpu', i)),
			none,
			0
		);
		expect(m.frame(0)).toHaveLength(2);
		m.setCapacity(3);
		expect(m.frame(0)).toHaveLength(3);
	});
});
