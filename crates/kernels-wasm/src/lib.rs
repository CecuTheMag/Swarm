//! Reference kernels exported for the browser's CPU fallback.
//!
//! Built with `cargo build -p kernels-wasm --target wasm32-unknown-unknown --release`
//! (the web build does this). The module has no imports and no memory use beyond
//! the stack, so the browser instantiates it with an empty import object.
//!
//! WebAssembly has no unsigned types: `u32` arguments and results cross the
//! boundary as `i32` with the same bits, so JavaScript passes them as numbers and
//! reads results with `>>> 0`.

/// Samples `start..end` of a π unit inside the quarter circle
/// (see [`kernels::pi::count_inside_range`]).
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn pi_count_inside_range(seed: u32, unit_index: u32, start: u32, end: u32) -> u32 {
    kernels::pi::count_inside_range(seed, unit_index, start, end)
}
