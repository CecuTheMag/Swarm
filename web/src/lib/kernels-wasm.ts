// The Rust reference kernels compiled to WebAssembly (crates/kernels-wasm): the CPU
// fallback for devices without WebGPU, and the console's timetable scoring. The Vite
// config builds it. Its own module, so the console loads it without the shaders.
export { default as kernelsWasmUrl } from '../../../target/wasm32-unknown-unknown/release/kernels_wasm.wasm?url';
