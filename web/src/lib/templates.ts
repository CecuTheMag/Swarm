// Kernel sources and test vectors live in /templates, shared with the Rust
// coordinator and reference kernels; the site bundles exactly these files.
export { default as piShader } from '../../../templates/pi/pi.wgsl?raw';
export { default as piVectors } from '../../../templates/pi/vectors.json';
export { default as ptShader } from '../../../templates/pt/pt.wgsl?raw';
export { default as ptVectors } from '../../../templates/pt/vectors.json';
export { default as nnShader } from '../../../templates/nn/nn.wgsl?raw';
export { default as nnVectors } from '../../../templates/nn/vectors.json';
// The timetable solver is Rust compiled to WebAssembly; its vectors pin its behaviour,
// and their hash is the template's (spec 0013).
export { default as ttVectorsText } from '../../../templates/tt/vectors.json?raw';
// `mnist` packed at 14 × 14 (tools/pack_mnist.py); devices download it once, cached.
export { default as mnistUrl } from '../../../templates/nn/mnist14.bin?url';
export { kernelsWasmUrl } from './kernels-wasm.ts';
