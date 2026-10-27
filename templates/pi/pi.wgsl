// Monte Carlo estimate of pi — GPU port of `kernels::pi` (crates/kernels/src/pi.rs).
// Must stay bit-identical to the Rust reference: same hash, same integer math.
// u32 arithmetic in WGSL wraps on overflow, matching Rust's wrapping_* calls.

struct Params {
  seed: u32,
  unit_index: u32,
  samples: u32,
  _pad: u32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read_write> inside: atomic<u32>;

// Workgroup memory is zero-initialised at dispatch start.
var<workgroup> local_count: atomic<u32>;

const WORKGROUP_SIZE: u32 = 256u;
const RADIUS_SQ: u32 = 1073741824u; // 32768^2

fn pcg_hash(input: u32) -> u32 {
  let state = input * 747796405u + 2891336453u;
  let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
  return (word >> 22u) ^ word;
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
  for (var i = gid.x; i < params.samples; i += stride) {
    let k = base + i * 2u;
    let x = pcg_hash(k) >> 17u;
    let y = pcg_hash(k + 1u) >> 17u;
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
