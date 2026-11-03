// Neural network sweep — GPU port of `kernels::nn` (crates/kernels/src/nn.rs), spec 0011.
// Must stay bit-identical to the Rust reference: integers only, the same shifts, the
// same tables. i32 and u32 arithmetic in WGSL wraps; the reference's bounds keep every
// value far from wrapping. Integer sums are exact in any order, so threads may add up
// a batch however the GPU schedules them.
//
// One training step is a fixed sequence of dispatches in one compute pass (each sees
// the writes of the previous one): pick → forward per layer → softmax_loss →
// backward per hidden layer → update per layer → finish_step. Evaluation runs
// forward per layer, then evaluate, over chunks of the validation images.

// Static for the whole run.
struct Run {
  steps: u32,
  batch: u32,
  batch_log2: u32,
  lr: i32,
  seed: u32,
  train_len: u32,
  dim: u32,
  layers: u32,
}

// Per dispatch, selected with a dynamic offset.
struct Pass {
  fan_in: u32,
  fan_out: u32,
  // Offset of the layer's weights in `weights` (biases follow them).
  w_off: u32,
  // Offset in `acts` of the layer's input; NO_ACTS means the pixels.
  in_off: u32,
  out_off: u32,
  // Offset in `deltas` of the error at this layer's output, and at its input.
  delta_off: u32,
  delta_in_off: u32,
  // 1 for hidden layers (ReLU).
  hidden: u32,
  // Images in this dispatch: the batch, or a chunk of the validation images.
  samples: u32,
  // 1 when evaluating; then image b is validation image `chunk + b`.
  eval: u32,
  chunk: u32,
}

struct State {
  step: u32,
  correct: atomic<u32>,
  curve_sum: array<u32, 16>,
  curve_steps: array<u32, 16>,
  // Training image of each batch slot, and its loss (Q16).
  idx: array<u32, 128>,
  ce: array<u32, 128>,
}

@group(0) @binding(0) var<uniform> run: Run;
@group(0) @binding(1) var<uniform> pass_: Pass;
// Training images then validation images, `dim` bytes each, four to a word.
@group(0) @binding(2) var<storage, read> pixels: array<u32>;
@group(0) @binding(3) var<storage, read> labels: array<u32>;
// Master weights and biases (Q24), layer after layer.
@group(0) @binding(4) var<storage, read_write> weights: array<i32>;
// Activations (Q8): per layer output, `samples × fan_out`.
@group(0) @binding(5) var<storage, read_write> acts: array<i32>;
// Errors (Q15): per layer output, `batch × fan_out`.
@group(0) @binding(6) var<storage, read_write> deltas: array<i32>;
@group(0) @binding(7) var<storage, read_write> state: State;

const WORKGROUP_SIZE: u32 = 64u;
const NO_ACTS: u32 = 0xffffffffu;
const CLASSES: u32 = 10u;
const CURVE_POINTS: u32 = 16u;
const A_MAX: i32 = 2047;        // just under 8.0 in Q8
const W_MAX: i32 = 67108864;    // 4.0 in Q24
const D_MAX: i32 = 65536;       // 2.0 in Q15
const G_MAX: i32 = 65535;       // just under 1.0 in Q16

const EXP2 = array<u32, 256>(
  32768u, 32857u, 32946u, 33035u, 33125u, 33215u, 33305u, 33395u, 33486u, 33576u, 33667u, 33759u,
  33850u, 33942u, 34034u, 34126u, 34219u, 34312u, 34405u, 34498u, 34591u, 34685u, 34779u, 34874u,
  34968u, 35063u, 35158u, 35253u, 35349u, 35445u, 35541u, 35637u, 35734u, 35831u, 35928u, 36025u,
  36123u, 36221u, 36319u, 36417u, 36516u, 36615u, 36715u, 36814u, 36914u, 37014u, 37114u, 37215u,
  37316u, 37417u, 37518u, 37620u, 37722u, 37824u, 37927u, 38030u, 38133u, 38236u, 38340u, 38444u,
  38548u, 38653u, 38757u, 38863u, 38968u, 39074u, 39180u, 39286u, 39392u, 39499u, 39606u, 39714u,
  39821u, 39929u, 40037u, 40146u, 40255u, 40364u, 40473u, 40583u, 40693u, 40804u, 40914u, 41025u,
  41136u, 41248u, 41360u, 41472u, 41584u, 41697u, 41810u, 41923u, 42037u, 42151u, 42265u, 42380u,
  42495u, 42610u, 42726u, 42841u, 42958u, 43074u, 43191u, 43308u, 43425u, 43543u, 43661u, 43780u,
  43898u, 44017u, 44137u, 44256u, 44376u, 44497u, 44617u, 44738u, 44859u, 44981u, 45103u, 45225u,
  45348u, 45471u, 45594u, 45718u, 45842u, 45966u, 46091u, 46216u, 46341u, 46467u, 46593u, 46719u,
  46846u, 46973u, 47100u, 47228u, 47356u, 47484u, 47613u, 47742u, 47871u, 48001u, 48131u, 48262u,
  48393u, 48524u, 48655u, 48787u, 48920u, 49052u, 49185u, 49319u, 49452u, 49586u, 49721u, 49856u,
  49991u, 50126u, 50262u, 50399u, 50535u, 50672u, 50810u, 50947u, 51085u, 51224u, 51363u, 51502u,
  51642u, 51782u, 51922u, 52063u, 52204u, 52346u, 52488u, 52630u, 52773u, 52916u, 53059u, 53203u,
  53347u, 53492u, 53637u, 53782u, 53928u, 54074u, 54221u, 54368u, 54515u, 54663u, 54811u, 54960u,
  55109u, 55258u, 55408u, 55558u, 55709u, 55860u, 56012u, 56163u, 56316u, 56468u, 56622u, 56775u,
  56929u, 57083u, 57238u, 57393u, 57549u, 57705u, 57861u, 58018u, 58176u, 58333u, 58491u, 58650u,
  58809u, 58968u, 59128u, 59289u, 59449u, 59611u, 59772u, 59934u, 60097u, 60260u, 60423u, 60587u,
  60751u, 60916u, 61081u, 61247u, 61413u, 61579u, 61746u, 61914u, 62081u, 62250u, 62419u, 62588u,
  62757u, 62928u, 63098u, 63269u, 63441u, 63613u, 63785u, 63958u, 64132u, 64306u, 64480u, 64655u,
  64830u, 65006u, 65182u, 65359u,
);

const LOG2 = array<u32, 256>(
  0u, 369u, 736u, 1102u, 1466u, 1829u, 2190u, 2551u, 2909u, 3267u, 3623u, 3978u,
  4331u, 4683u, 5034u, 5384u, 5732u, 6079u, 6425u, 6769u, 7112u, 7454u, 7795u, 8134u,
  8473u, 8810u, 9146u, 9480u, 9814u, 10146u, 10477u, 10807u, 11136u, 11464u, 11791u, 12116u,
  12440u, 12764u, 13086u, 13407u, 13727u, 14046u, 14363u, 14680u, 14996u, 15310u, 15624u, 15937u,
  16248u, 16559u, 16868u, 17177u, 17484u, 17791u, 18096u, 18401u, 18704u, 19007u, 19308u, 19609u,
  19909u, 20207u, 20505u, 20802u, 21098u, 21393u, 21687u, 21980u, 22272u, 22564u, 22854u, 23144u,
  23433u, 23720u, 24007u, 24293u, 24579u, 24863u, 25146u, 25429u, 25711u, 25992u, 26272u, 26551u,
  26830u, 27108u, 27384u, 27660u, 27936u, 28210u, 28484u, 28757u, 29029u, 29300u, 29571u, 29840u,
  30109u, 30378u, 30645u, 30912u, 31178u, 31443u, 31707u, 31971u, 32234u, 32496u, 32758u, 33019u,
  33279u, 33538u, 33797u, 34055u, 34312u, 34569u, 34825u, 35080u, 35334u, 35588u, 35841u, 36094u,
  36346u, 36597u, 36847u, 37097u, 37346u, 37595u, 37842u, 38090u, 38336u, 38582u, 38827u, 39072u,
  39316u, 39559u, 39802u, 40044u, 40286u, 40527u, 40767u, 41006u, 41246u, 41484u, 41722u, 41959u,
  42196u, 42432u, 42667u, 42902u, 43137u, 43370u, 43603u, 43836u, 44068u, 44300u, 44530u, 44761u,
  44990u, 45220u, 45448u, 45676u, 45904u, 46131u, 46357u, 46583u, 46809u, 47034u, 47258u, 47482u,
  47705u, 47928u, 48150u, 48372u, 48593u, 48813u, 49034u, 49253u, 49472u, 49691u, 49909u, 50127u,
  50344u, 50560u, 50776u, 50992u, 51207u, 51422u, 51636u, 51850u, 52063u, 52276u, 52488u, 52700u,
  52911u, 53122u, 53332u, 53542u, 53751u, 53960u, 54169u, 54377u, 54584u, 54791u, 54998u, 55204u,
  55410u, 55615u, 55820u, 56025u, 56229u, 56432u, 56635u, 56838u, 57040u, 57242u, 57443u, 57644u,
  57845u, 58045u, 58245u, 58444u, 58643u, 58841u, 59039u, 59237u, 59434u, 59631u, 59827u, 60023u,
  60219u, 60414u, 60609u, 60803u, 60997u, 61190u, 61384u, 61576u, 61769u, 61961u, 62152u, 62343u,
  62534u, 62725u, 62915u, 63104u, 63294u, 63483u, 63671u, 63859u, 64047u, 64234u, 64421u, 64608u,
  64794u, 64980u, 65166u, 65351u,
);

// Hashes: see crates/kernels/src/rng.rs.
fn pcg_hash(input: u32) -> u32 {
  let state = input * 747796405u + 2891336453u;
  let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
  return (word >> 22u) ^ word;
}

fn pcg2d(a: u32, b: u32) -> vec2<u32> {
  var v = vec2<u32>(a, b) * 1664525u + 1013904223u;
  v.x += v.y * 1664525u;
  v.y += v.x * 1664525u;
  v = v ^ (v >> vec2<u32>(16u));
  v.x += v.y * 1664525u;
  v.y += v.x * 1664525u;
  v = v ^ (v >> vec2<u32>(16u));
  return v;
}

fn lowbias32(input: u32) -> u32 {
  var x = input;
  x ^= x >> 16u;
  x *= 0x7feb352du;
  x ^= x >> 15u;
  x *= 0x846ca68bu;
  x ^= x >> 16u;
  return x;
}

fn unit_base(seed: u32, unit_index: u32) -> u32 {
  return pcg_hash(seed ^ pcg_hash(unit_index));
}

// High 32 bits of a × b (WGSL has no 64-bit integers).
fn mul_hi(a: u32, b: u32) -> u32 {
  let a_lo = a & 0xffffu;
  let a_hi = a >> 16u;
  let b_lo = b & 0xffffu;
  let b_hi = b >> 16u;
  let lo_lo = a_lo * b_lo;
  let mid1 = a_hi * b_lo;
  let mid2 = a_lo * b_hi;
  let carry = ((lo_lo >> 16u) + (mid1 & 0xffffu) + (mid2 & 0xffffu)) >> 16u;
  return a_hi * b_hi + (mid1 >> 16u) + (mid2 >> 16u) + carry;
}

// Master weight (Q24) rounded to the forward format (Q12).
fn forward_weight(m: i32) -> i32 {
  return (m + 2048) >> 12u;
}

// Image number of slot `b` in this dispatch, counting training images first.
fn image_of(b: u32) -> u32 {
  if (pass_.eval == 1u) {
    return run.train_len + pass_.chunk + b;
  }
  return state.idx[b];
}

fn pixel(image: u32, i: u32) -> i32 {
  let byte = image * run.dim + i;
  return i32((pixels[byte >> 2u] >> ((byte & 3u) * 8u)) & 255u);
}

fn label(image: u32) -> u32 {
  return (labels[image >> 2u] >> ((image & 3u) * 8u)) & 255u;
}

// Input activation i of slot b for this dispatch's layer.
fn input_act(b: u32, i: u32) -> i32 {
  if (pass_.in_off == NO_ACTS) {
    return pixel(image_of(b), i);
  }
  return acts[pass_.in_off + b * pass_.fan_in + i];
}

// Which training image fills each batch slot at this step (kernels::nn::batch_index).
@compute @workgroup_size(WORKGROUP_SIZE)
fn pick(@builtin(global_invocation_id) gid: vec3<u32>) {
  let b = gid.x;
  if (b >= run.batch) {
    return;
  }
  let v = pcg2d(state.step * run.batch + b, unit_base(run.seed, 0x2000u));
  state.idx[b] = mul_hi(lowbias32(v.x), run.train_len);
}

// One output of one layer for one image.
@compute @workgroup_size(WORKGROUP_SIZE)
fn forward(@builtin(global_invocation_id) gid: vec3<u32>) {
  let t = gid.x;
  if (t >= pass_.samples * pass_.fan_out) {
    return;
  }
  let b = t / pass_.fan_out;
  let o = t % pass_.fan_out;
  var acc = 0;
  for (var i = 0u; i < pass_.fan_in; i++) {
    acc += (input_act(b, i) * forward_weight(weights[pass_.w_off + i * pass_.fan_out + o])) >> 4u;
  }
  let bias = weights[pass_.w_off + pass_.fan_in * pass_.fan_out + o];
  var z = (acc >> 8u) + (bias >> 16u);
  if (pass_.hidden == 1u) {
    z = clamp(z, 0, A_MAX);
  }
  acts[pass_.out_off + b * pass_.fan_out + o] = z;
}

// Softmax, loss and the output error p − y for one image of the batch.
@compute @workgroup_size(WORKGROUP_SIZE)
fn softmax_loss(@builtin(global_invocation_id) gid: vec3<u32>) {
  let b = gid.x;
  if (b >= run.batch) {
    return;
  }
  let base = pass_.out_off + b * CLASSES;
  var top = acts[base];
  for (var k = 1u; k < CLASSES; k++) {
    top = max(top, acts[base + k]);
  }
  var e: array<u32, 10>;
  var sum = 0u;
  for (var k = 0u; k < CLASSES; k++) {
    let d = max(acts[base + k] - top, -8192);
    let u = (d * 23637) >> 14u;
    let q = u >> 8u;
    let f = u32(u & 255);
    if (-q > 31) {
      e[k] = 0u;
    } else {
      e[k] = EXP2[f] >> u32(-q);
    }
    sum += e[k];
  }
  let y = label(state.idx[b]);
  for (var k = 0u; k < CLASSES; k++) {
    let p = i32((e[k] << 15u) / sum);
    var truth = 0;
    if (k == y) {
      truth = 32768;
      // Cross-entropy −ln p in Q16 (kernels::nn::cross_entropy).
      let x = u32(clamp(p, 1, 32768));
      let lz = 31u - countLeadingZeros(x);
      let log2_x = (lz << 16u) + LOG2[((x << 8u) >> lz) & 255u];
      state.ce[b] = ((((15u << 16u) - log2_x) >> 4u) * 45426u) >> 12u;
    }
    deltas[pass_.delta_off + b * CLASSES + k] = p - truth;
  }
}

// Error at input i of a hidden layer for one image, through ReLU and the clamp.
@compute @workgroup_size(WORKGROUP_SIZE)
fn backward(@builtin(global_invocation_id) gid: vec3<u32>) {
  let t = gid.x;
  if (t >= run.batch * pass_.fan_in) {
    return;
  }
  let b = t / pass_.fan_in;
  let i = t % pass_.fan_in;
  var d = 0;
  for (var o = 0u; o < pass_.fan_out; o++) {
    d += (deltas[pass_.delta_off + b * pass_.fan_out + o]
      * forward_weight(weights[pass_.w_off + i * pass_.fan_out + o])) >> 12u;
  }
  let a = acts[pass_.in_off + b * pass_.fan_in + i];
  var out = 0;
  if (a > 0 && a < A_MAX) {
    out = clamp(d, -D_MAX, D_MAX);
  }
  deltas[pass_.delta_in_off + b * pass_.fan_in + i] = out;
}

fn sgd(m: i32, sum: i32) -> i32 {
  let g = clamp(sum >> run.batch_log2, -G_MAX, G_MAX);
  return clamp(m - ((run.lr * g + 128) >> 8u), -W_MAX, W_MAX);
}

// Mean gradient over the batch and the update, for one weight or bias.
@compute @workgroup_size(WORKGROUP_SIZE)
fn update(@builtin(global_invocation_id) gid: vec3<u32>) {
  let t = gid.x;
  let n_w = pass_.fan_in * pass_.fan_out;
  if (t >= n_w + pass_.fan_out) {
    return;
  }
  var sum = 0;
  if (t < n_w) {
    let i = t / pass_.fan_out;
    let o = t % pass_.fan_out;
    for (var b = 0u; b < run.batch; b++) {
      sum += (deltas[pass_.delta_off + b * pass_.fan_out + o] * input_act(b, i)) >> 7u;
    }
  } else {
    let o = t - n_w;
    for (var b = 0u; b < run.batch; b++) {
      sum += deltas[pass_.delta_off + b * pass_.fan_out + o] << 1u;
    }
  }
  weights[pass_.w_off + t] = sgd(weights[pass_.w_off + t], sum);
}

// The step's mean loss into the curve, and on to the next step.
@compute @workgroup_size(1)
fn finish_step() {
  var loss = 0u;
  for (var b = 0u; b < run.batch; b++) {
    loss += state.ce[b];
  }
  let point = (state.step * CURVE_POINTS) / run.steps;
  state.curve_sum[point] += loss >> run.batch_log2;
  state.curve_steps[point] += 1u;
  state.step += 1u;
}

// Counts validation images whose largest logit (the first on a tie) is their label.
@compute @workgroup_size(WORKGROUP_SIZE)
fn evaluate(@builtin(global_invocation_id) gid: vec3<u32>) {
  let b = gid.x;
  if (b >= pass_.samples) {
    return;
  }
  let base = pass_.out_off + b * CLASSES;
  var best = 0u;
  for (var k = 1u; k < CLASSES; k++) {
    if (acts[base + k] > acts[base + best]) {
      best = k;
    }
  }
  if (best == label(image_of(b))) {
    atomicAdd(&state.correct, 1u);
  }
}
