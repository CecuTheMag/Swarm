// Path tracer (spec 0003). One invocation per pixel of a tile adds samples
// `first .. first + count` to the pixel's running radiance sums. Mirrors the Rust
// reference `kernels::pt` step by step, including adding samples one at a time in
// order, so results differ from it only by GPU floating-point details.

struct Params {
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    first: u32,
    count: u32,
    seed: u32,
}

@group(0) @binding(0) var<uniform> params: Params;
// w × h × 3 running sums, row-major RGB.
@group(0) @binding(1) var<storage, read_write> sums: array<f32>;

const MAX_DEPTH: u32 = 6u;
const EMISSION = vec3<f32>(10.0, 8.5, 6.0);
const EPS: f32 = 1e-4;
const TAN_HALF_FOV: f32 = 0.33;
const CAMERA = vec3<f32>(0.0, 0.9, 3.6);
const BOX_FRONT: f32 = 3.5;
const LIGHT_HALF: f32 = 0.3;
const RED = vec3<f32>(0.63, 0.065, 0.05);
const GREEN = vec3<f32>(0.14, 0.45, 0.091);
const WHITE = vec3<f32>(0.725, 0.71, 0.68);
const MIRROR_TINT: f32 = 0.95;
const GLASS_IOR: f32 = 1.5;

const DIFFUSE: u32 = 0u;
const LIGHT: u32 = 1u;
const MIRROR: u32 = 2u;
const GLASS: u32 = 3u;

fn pcg_hash(input: u32) -> u32 {
    let state = input * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn lowbias32(v: u32) -> u32 {
    var x = v;
    x ^= x >> 16u;
    x *= 0x7feb352du;
    x ^= x >> 15u;
    x *= 0x846ca68bu;
    x ^= x >> 16u;
    return x;
}

var<private> rng_state: u32;

fn rng_init(seed: u32, pixel: u32, sample: u32) {
    rng_state = pcg_hash(seed ^ pcg_hash(pixel ^ pcg_hash(sample)));
}

fn next_f32() -> f32 {
    rng_state += 0x9e3779b9u;
    return f32(lowbias32(rng_state) >> 8u) * (1.0 / 16777216.0);
}

struct Hit {
    found: bool,
    t: f32,
    normal: vec3<f32>,
    albedo: vec3<f32>,
    material: u32,
}

fn sphere(o: vec3<f32>, d: vec3<f32>, centre: vec3<f32>, radius: f32, material: u32, best: ptr<function, Hit>) {
    let oc = o - centre;
    let b = dot(oc, d);
    let c = dot(oc, oc) - radius * radius;
    let disc = b * b - c;
    if (disc <= 0.0) {
        return;
    }
    let root = sqrt(disc);
    var t = -b - root;
    if (t <= EPS) {
        t = -b + root;
    }
    if (t > EPS && t < (*best).t) {
        *best = Hit(true, t, (o + d * t - centre) * (1.0 / radius), WHITE, material);
    }
}

fn within(v: f32, lo: f32, hi: f32) -> bool {
    return v >= lo - 1e-4 && v <= hi + 1e-4;
}

fn in_box_face(p: vec3<f32>, axis: u32) -> bool {
    return (axis == 0u || within(p.x, -1.0, 1.0))
        && (axis == 1u || within(p.y, 0.0, 2.0))
        && (axis == 2u || within(p.z, -1.0, BOX_FRONT));
}

fn wall(o: vec3<f32>, d: vec3<f32>, axis: u32, plane: f32, albedo: vec3<f32>, best: ptr<function, Hit>) {
    let da = d[axis];
    if (da == 0.0) {
        return;
    }
    let t = (plane - o[axis]) / da;
    if (t <= EPS || t >= (*best).t) {
        return;
    }
    let p = o + d * t;
    if (!in_box_face(p, axis)) {
        return;
    }
    var normal = vec3<f32>(0.0);
    normal[axis] = select(1.0, -1.0, da > 0.0);
    let is_light = axis == 1u && plane == 2.0 && abs(p.x) < LIGHT_HALF && abs(p.z) < LIGHT_HALF;
    if (is_light && da < 0.0) {
        normal = vec3<f32>(0.0, 1.0, 0.0);
    }
    *best = Hit(true, t, normal, albedo, select(DIFFUSE, LIGHT, is_light && da > 0.0));
}

fn intersect(o: vec3<f32>, d: vec3<f32>) -> Hit {
    var best = Hit(false, 3.0e38, vec3<f32>(0.0), vec3<f32>(0.0), DIFFUSE);
    sphere(o, d, vec3<f32>(-0.42, 0.4, -0.3), 0.4, MIRROR, &best);
    sphere(o, d, vec3<f32>(0.45, 0.33, 0.35), 0.33, GLASS, &best);
    wall(o, d, 0u, -1.0, RED, &best);
    wall(o, d, 0u, 1.0, GREEN, &best);
    wall(o, d, 1u, 0.0, WHITE, &best);
    wall(o, d, 1u, 2.0, WHITE, &best);
    wall(o, d, 2u, -1.0, WHITE, &best);
    return best;
}

// As in the Rust reference (not the builtin), to stay as close to it as possible.
fn unit(v: vec3<f32>) -> vec3<f32> {
    return v * (1.0 / sqrt(dot(v, v)));
}

fn reflect_dir(d: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    return d - n * (2.0 * dot(d, n));
}

fn random_unit_vector() -> vec3<f32> {
    var v = vec3<f32>(0.0, 1.0, 0.0);
    for (var i = 0u; i < 16u; i++) {
        let c = vec3<f32>(next_f32() * 2.0 - 1.0, next_f32() * 2.0 - 1.0, next_f32() * 2.0 - 1.0);
        let len2 = dot(c, c);
        if (len2 > 1e-6 && len2 <= 1.0) {
            v = c * (1.0 / sqrt(len2));
            break;
        }
    }
    return v;
}

fn trace(start: vec3<f32>, start_dir: vec3<f32>) -> vec3<f32> {
    var origin = start;
    var dir = start_dir;
    var throughput = vec3<f32>(1.0);
    var radiance = vec3<f32>(0.0);
    for (var depth = 0u; depth < MAX_DEPTH; depth++) {
        let hit = intersect(origin, dir);
        if (!hit.found) {
            break;
        }
        let point = origin + dir * hit.t;
        if (hit.material == LIGHT) {
            radiance += throughput * EMISSION;
            break;
        }
        if (hit.material == DIFFUSE) {
            throughput *= hit.albedo;
            let bounced = hit.normal + random_unit_vector();
            let len2 = dot(bounced, bounced);
            dir = select(hit.normal, bounced * (1.0 / sqrt(len2)), len2 > 1e-12);
            origin = point + hit.normal * EPS;
        } else if (hit.material == MIRROR) {
            throughput *= MIRROR_TINT;
            dir = reflect_dir(dir, hit.normal);
            origin = point + hit.normal * EPS;
        } else {
            let entering = -dot(dir, hit.normal) > 0.0;
            let normal = select(-hit.normal, hit.normal, entering);
            let eta = select(GLASS_IOR, 1.0 / GLASS_IOR, entering);
            let cos_i = -dot(dir, normal);
            let k = 1.0 - eta * eta * (1.0 - cos_i * cos_i);
            var r0 = (1.0 - GLASS_IOR) / (1.0 + GLASS_IOR);
            r0 = r0 * r0;
            let m = 1.0 - cos_i;
            let fresnel = r0 + (1.0 - r0) * (m * m * m * m * m);
            if (k < 0.0 || next_f32() < fresnel) {
                dir = reflect_dir(dir, normal);
                origin = point + normal * EPS;
            } else {
                dir = unit(dir * eta + normal * (eta * cos_i - sqrt(k)));
                origin = point - normal * EPS;
            }
        }
    }
    return radiance;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= params.w || gid.y >= params.h) {
        return;
    }
    let px = params.x + gid.x;
    let py = params.y + gid.y;
    let pixel = py * params.width + px;
    let scale = TAN_HALF_FOV / f32(params.height);
    let at = (gid.y * params.w + gid.x) * 3u;
    var sum = vec3<f32>(sums[at], sums[at + 1u], sums[at + 2u]);
    for (var k = 0u; k < params.count; k++) {
        rng_init(params.seed, pixel, params.first + k);
        let jx = next_f32();
        let jy = next_f32();
        let u = (2.0 * (f32(px) + jx) - f32(params.width)) * scale;
        let v = (f32(params.height) - 2.0 * (f32(py) + jy)) * scale;
        sum += trace(CAMERA, unit(vec3<f32>(u, v, -1.0)));
    }
    sums[at] = sum.x;
    sums[at + 1u] = sum.y;
    sums[at + 2u] = sum.z;
}
