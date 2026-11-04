// Kernel sources and test vectors live in /templates, shared with the Rust
// coordinator and reference kernels; the site bundles exactly these files.
export { default as piShader } from '../../../templates/pi/pi.wgsl?raw';
export { default as piVectors } from '../../../templates/pi/vectors.json';
export { default as ptShader } from '../../../templates/pt/pt.wgsl?raw';
export { default as ptVectors } from '../../../templates/pt/vectors.json';
export { default as nnShader } from '../../../templates/nn/nn.wgsl?raw';
export { default as nnVectors } from '../../../templates/nn/vectors.json';
// `mnist` packed at 14 × 14 (tools/pack_mnist.py); devices download it once, cached.
export { default as mnistUrl } from '../../../templates/nn/mnist14.bin?url';
// The Rust reference kernels compiled to WebAssembly (crates/kernels-wasm), the
// CPU fallback for devices without WebGPU. The Vite config builds it.
export { default as kernelsWasmUrl } from '../../../target/wasm32-unknown-unknown/release/kernels_wasm.wasm?url';
