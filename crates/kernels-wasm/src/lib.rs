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

/// `glyphs`, and the same images as the browser uploads them to the GPU: pixels of
/// the training then validation images, and their labels in the same order.
struct Glyphs {
    data: kernels::nn::Dataset,
    pixels: Vec<u8>,
    labels: Vec<u8>,
}

thread_local! {
    /// Generated on first use.
    static GLYPHS: std::cell::OnceCell<Glyphs> = const { std::cell::OnceCell::new() };
    /// Holds the last result of [`nn_init`] or [`nn_train`].
    static NN_OUT: std::cell::RefCell<Vec<u32>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn with_glyphs<T>(f: impl FnOnce(&Glyphs) -> T) -> T {
    GLYPHS.with(|cell| {
        f(cell.get_or_init(|| {
            let data = kernels::nn::glyphs::dataset();
            let pixels = [data.train_x.as_slice(), &data.val_x].concat();
            let labels = [data.train_y.as_slice(), &data.val_y].concat();
            Glyphs {
                data,
                pixels,
                labels,
            }
        }))
    })
}

/// Pixels of `glyphs` (training images, then validation images; 196 bytes each).
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn nn_glyphs_pixels() -> *const u8 {
    with_glyphs(|g| g.pixels.as_ptr())
}

/// Labels of `glyphs`, in the order of [`nn_glyphs_pixels`].
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn nn_glyphs_labels() -> *const u8 {
    with_glyphs(|g| g.labels.as_ptr())
}

fn weight_words(layers: &[kernels::nn::Layer], out: &mut Vec<u32>) {
    out.extend(
        layers
            .iter()
            .flat_map(|l| l.w.iter().chain(&l.b))
            .map(|&v| v as u32),
    );
}

/// Initial master weights of a configuration (see [`kernels::nn::init_layers`]):
/// per layer its weights, then its biases. Valid until the next call.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn nn_init(hidden: u32, layers: u32, seed: u32) -> *const u32 {
    let cfg = kernels::nn::Config {
        hidden,
        layers,
        lr_q16: 0,
        batch_log2: 0,
        steps: 1,
        seed,
    };
    NN_OUT.with_borrow_mut(|out| {
        out.clear();
        weight_words(
            &kernels::nn::init_layers(&cfg, kernels::nn::glyphs::DIM),
            out,
        );
        out.as_ptr()
    })
}

/// Trains a configuration on `glyphs` (see [`kernels::nn::train`]). The result is
/// the number of correct validation images, the 16 curve points, then the final
/// weights as for [`nn_init`]; valid until the next call.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn nn_train(
    hidden: u32,
    layers: u32,
    lr_q16: u32,
    batch_log2: u32,
    steps: u32,
    seed: u32,
) -> *const u32 {
    let cfg = kernels::nn::Config {
        hidden,
        layers,
        lr_q16,
        batch_log2,
        steps,
        seed,
    };
    let trained = with_glyphs(|g| kernels::nn::train(&cfg, &g.data));
    NN_OUT.with_borrow_mut(|out| {
        out.clear();
        out.push(trained.correct);
        out.extend(trained.curve);
        weight_words(&trained.layers, out);
        out.as_ptr()
    })
}
