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

thread_local! {
    /// Holds the last tile rendered by [`pt_render_rows`]; JavaScript reads it
    /// through the module's memory.
    static PT_OUT: std::cell::RefCell<Vec<f32>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Renders rows `row_start..row_end` of a path-tracer tile (see
/// [`kernels::pt::render_rows`]) and returns a pointer to the
/// `(row_end − row_start) × w × 3` radiance sums in this module's memory. The data
/// stays valid until the next call.
#[allow(unsafe_code, clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub extern "C" fn pt_render_rows(
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    spp: u32,
    pass: u32,
    seed: u32,
    row_start: u32,
    row_end: u32,
) -> *const f32 {
    let params = kernels::pt::TileParams {
        width,
        height,
        x,
        y,
        w,
        h,
        spp,
        pass,
        seed,
    };
    PT_OUT.with_borrow_mut(|out| {
        out.clear();
        out.resize((row_end - row_start) as usize * w as usize * 3, 0.0);
        kernels::pt::render_rows(&params, row_start, row_end, out);
        out.as_ptr()
    })
}
