// Trying out a trained network (spec 0011): a drawing becomes a 14 × 14 digit drawn
// the way the `glyphs` dataset draws its own (crates/kernels/src/nn/glyphs.rs), and
// the network reads it with the devices' integer arithmetic (kernels::nn::forward),
// so it answers exactly as it was scored.

export const SIDE = 14;
/** Activations are Q8, clamped to 0..8 (kernels::nn::A_MAX). */
const A_MAX = 2047;
/** Stroke half-width in pixels: the middle of the dataset's widths (12 × 0.045..0.13). */
const HALF_WIDTH = 1.05;
/** Share of the box the drawing's longer side fills, as the dataset's digits do. */
const FILL = 0.85;

export interface Point {
	x: number;
	y: number;
}

/** One layer of a downloaded model (coordinator's networks::ModelLayer). */
export interface ModelLayer {
	inputs: number;
	outputs: number;
	activation: string;
	weights: number[];
	biases: number[];
}

export interface Model {
	hidden: number;
	layers: number;
	learning_rate: number;
	batch: number;
	accuracy: number;
	digest: string;
	network: ModelLayer[];
}

function segmentDistanceSq(p: Point, a: Point, b: Point): number {
	const dx = b.x - a.x;
	const dy = b.y - a.y;
	const len = dx * dx + dy * dy;
	const t = len > 0 ? Math.min(1, Math.max(0, ((p.x - a.x) * dx + (p.y - a.y) * dy) / len)) : 0;
	const ex = p.x - (a.x + t * dx);
	const ey = p.y - (a.y + t * dy);
	return ex * ex + ey * ey;
}

/**
 * The 14 × 14 grey pixels (0..255, row by row) of strokes drawn anywhere: centred and
 * scaled into the dataset's box with a one-pixel margin, drawn with its pen.
 */
export function rasterize(strokes: Point[][]): Uint8Array {
	const out = new Uint8Array(SIDE * SIDE);
	const points = strokes.flat();
	if (points.length === 0) return out;
	const xs = points.map((p) => p.x);
	const ys = points.map((p) => p.y);
	const [x0, x1, y0, y1] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
	const size = Math.max(x1 - x0, y1 - y0) || 1;
	const toBox = (p: Point): Point => ({
		x: 1 + 12 * (0.5 + ((p.x - (x0 + x1) / 2) / size) * FILL),
		y: 1 + 12 * (0.5 + ((p.y - (y0 + y1) / 2) / size) * FILL)
	});
	const segments: [Point, Point][] = [];
	for (const stroke of strokes) {
		const box = stroke.map(toBox);
		if (box.length === 1) segments.push([box[0], box[0]]);
		for (let i = 0; i + 1 < box.length; i++) segments.push([box[i], box[i + 1]]);
	}
	for (let py = 0; py < SIDE; py++) {
		for (let px = 0; px < SIDE; px++) {
			const p = { x: px + 0.5, y: py + 0.5 };
			let d = Infinity;
			for (const [a, b] of segments) d = Math.min(d, segmentDistanceSq(p, a, b));
			const ink = Math.min(1, Math.max(0, 1 - (Math.sqrt(d) - HALF_WIDTH)));
			out[py * SIDE + px] = Math.floor(ink * 255 + 0.5);
		}
	}
	return out;
}

/** What the network makes of a digit: its answer and how sure it is of each. */
export interface Reading {
	digit: number;
	probabilities: number[];
}

/**
 * The network of a downloaded model as the devices run it: master weights back to Q24
 * integers (the download divides them by 2²⁴, exactly), then the integer forward pass.
 */
export function networkOf(model: Model): (pixels: Uint8Array) => Reading {
	const layers = model.network.map((layer) => ({
		inputs: layer.inputs,
		outputs: layer.outputs,
		// kernels::nn::forward_weight: Q24 master weights rounded to Q12.
		w: Int32Array.from(layer.weights, (v) => (Math.round(v * 2 ** 24) + 2048) >> 12),
		b: Int32Array.from(layer.biases, (v) => Math.round(v * 2 ** 24) >> 16)
	}));
	return (pixels) => {
		let a = Int32Array.from(pixels);
		layers.forEach((layer, l) => {
			const hidden = l + 1 < layers.length;
			const out = new Int32Array(layer.outputs);
			for (let o = 0; o < layer.outputs; o++) {
				let acc = 0; // Q16
				for (let i = 0; i < layer.inputs; i++) acc += (a[i] * layer.w[i * layer.outputs + o]) >> 4;
				const z = (acc >> 8) + layer.b[o];
				out[o] = hidden ? Math.min(A_MAX, Math.max(0, z)) : z;
			}
			a = out;
		});
		// The logits are Q8; the probabilities are for display only.
		const top = Math.max(...a);
		const e = Array.from(a, (z) => Math.exp((z - top) / 256));
		const sum = e.reduce((s, v) => s + v, 0);
		return { digit: a.indexOf(top), probabilities: e.map((v) => v / sum) };
	};
}
