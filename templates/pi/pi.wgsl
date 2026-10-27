// Monte Carlo estimate of pi — GPU port of `kernels::pi` (crates/kernels/src/pi.rs).
// Must stay bit-identical to the Rust reference: same hash, same integer math.
// u32 arithmetic in WGSL wraps on overflow, matching Rust's wrapping_* calls.

// One dispatch covers samples [start, end) of a unit, so a large unit can be
// split into short dispatches (GPU watchdogs kill long-running ones).
struct Params {
  seed: u32,
  unit_index: u32,
  start: u32,
  end: u32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read_write> inside: atomic<u32>;

// Workgroup memory is zero-initialised at dispatch start.
var<workgroup> local_count: atomic<u32>;

const WORKGROUP_SIZE: u32 = 256u;
const RADIUS_SQ: u32 = 1073741824u; // 32768^2

// Hashes: see crates/kernels/src/rng.rs for sources and the statistical tests
// behind this choice.
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

// Two independent uniform u32 values for sample i of the unit with this base.
fn sample2(base: u32, i: u32) -> vec2<u32> {
  let v = pcg2d(i, base);
  return vec2<u32>(lowbias32(v.x), lowbias32(v.y));
}

@compute @workgroup_size(WORKGROUP_SIZE)
fn main(
  @builtin(global_invocation_id) gid: vec3<u32>,
  @builtin(local_invocation_index) lid: u32,
  @builtin(num_workgroups) groups: vec3<u32>,
) {
  let base = pcg_hash(params.seed ^ pcg_hash(params.unit_index));
  let stride = groups.x * WORKGROUP_SIZE;

  var count = 0u;
  for (var i = params.start + gid.x; i < params.end; i += stride) {
    // Centre of a cell in a 16384 x 16384 grid, in doubled coordinates.
    let r = sample2(base, i);
    let x = 2u * (r.x >> 18u) + 1u;
    let y = 2u * (r.y >> 18u) + 1u;
    if (x * x + y * y < RADIUS_SQ) {
      count += 1u;
    }
  }

  // Integer addition is associative, so the total is the same on every device
  // regardless of the order in which workgroups finish.
  atomicAdd(&local_count, count);
  workgroupBarrier();
  if (lid == 0u) {
    atomicAdd(&inside, atomicLoad(&local_count));
  }
}
