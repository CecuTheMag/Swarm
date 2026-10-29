// Path-tracer helpers shared by the GPU and CPU kernels and the self-test (spec 0003).

/** Base64 of little-endian `f32` values: the `rgb` field of a unit's output. */
export function encodeRgb(values: Float32Array): string {
	const bytes = new Uint8Array(values.length * 4);
	const view = new DataView(bytes.buffer);
	values.forEach((v, i) => view.setFloat32(i * 4, v, true));
	let binary = '';
	const CHUNK = 0x8000;
	for (let i = 0; i < bytes.length; i += CHUNK) {
		binary += String.fromCharCode(...bytes.subarray(i, i + CHUNK));
	}
	return btoa(binary);
}

export function decodeRgb(text: string): Float32Array {
	const binary = atob(text);
	const view = new DataView(new ArrayBuffer(binary.length));
	for (let i = 0; i < binary.length; i++) view.setUint8(i, binary.charCodeAt(i));
	return Float32Array.from({ length: binary.length / 4 }, (_, i) => view.getFloat32(i * 4, true));
}

export interface Agreement {
	/** Values within 0.1% (or 0.001 absolute) of the reference. */
	close: number;
	total: number;
	/** Relative difference of the summed radiance. */
	sumError: number;
}

/**
 * How closely sums from a GPU follow the reference. GPUs may fuse or approximate
 * floating-point operations, so now and then a sample takes another branch (say at a
 * glass surface) and its pixel differs; the rest agree to rounding.
 */
export function agreement(actual: ArrayLike<number>, expected: ArrayLike<number>): Agreement {
	let close = 0;
	let sumA = 0;
	let sumE = 0;
	for (let i = 0; i < expected.length; i++) {
		const a = actual[i];
		const e = expected[i];
		if (Math.abs(a - e) <= Math.max(1e-3, 1e-3 * Math.abs(e))) close++;
		sumA += a;
		sumE += e;
	}
	return {
		close,
		total: expected.length,
		sumError: sumE === 0 ? Math.abs(sumA) : Math.abs(sumA - sumE) / sumE
	};
}

/** GPU results pass when at least 90% of values are close and the total is within 5%. */
export function gpuAgrees(a: Agreement): boolean {
	return a.total > 0 && a.close >= 0.9 * a.total && a.sumError <= 0.05;
}
