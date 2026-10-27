// Kernel sources and test vectors live in /templates, shared with the Rust
// coordinator and reference kernels; the site bundles exactly these files.
export { default as piShader } from '../../../templates/pi/pi.wgsl?raw';
export { default as piVectors } from '../../../templates/pi/vectors.json';
