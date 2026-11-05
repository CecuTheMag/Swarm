// The interface between Swarm and a program (spec 0008): four exports, no imports.

/** Largest output of one unit, as the coordinator accepts it. */
export const MAX_OUTPUT_BYTES = 256 * 1024;

interface ProgramExports {
	memory: WebAssembly.Memory;
	swarm_input(len: number): number;
	swarm_run(): number;
	swarm_output(): number;
}

/** Bytes `at..at + len` of `memory`, or an error the contributor can read. */
function view(memory: WebAssembly.Memory, at: number, len: number): Uint8Array {
	// Read the buffer anew each time: the program may have grown its memory, which
	// replaces it.
	if (at + len > memory.buffer.byteLength) {
		throw new Error('the program pointed outside its memory');
	}
	return new Uint8Array(memory.buffer, at, len);
}

/**
 * Runs one unit: a fresh instance of `module`, so nothing carries over from one unit
 * to the next. The import object is empty: a program cannot call anything but itself.
 * Pointers and lengths are unsigned, so they are read with `>>> 0`.
 */
export async function runUnit(module: WebAssembly.Module, input: Uint8Array): Promise<Uint8Array> {
	const instance = await WebAssembly.instantiate(module, {});
	const program = instance.exports as unknown as ProgramExports;
	const at = program.swarm_input(input.length) >>> 0;
	view(program.memory, at, input.length).set(input);
	const len = program.swarm_run();
	if (len < 0) throw new Error(`the program returned error code ${len}`);
	if (len > MAX_OUTPUT_BYTES) {
		throw new Error(`the output is ${len} bytes; at most ${MAX_OUTPUT_BYTES} are allowed`);
	}
	return view(program.memory, program.swarm_output() >>> 0, len).slice();
}
