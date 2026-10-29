//! Path tracer — the showpiece template (spec 0003).
//!
//! Renders a Cornell box with a mirror and a glass sphere by unidirectional path
//! tracing. The WGSL kernel (`templates/pt/pt.wgsl`) implements exactly the same
//! steps. Only `+ − × ÷ √` are used, all in `f32`, so native and WebAssembly builds
//! produce bit-identical sums; GPUs may differ slightly (fused or approximate
//! operations) and are compared with a tolerance.

use crate::rng::{lowbias32, pcg_hash};

/// Maximum path length (camera ray plus bounces).
pub const MAX_DEPTH: u32 = 6;
/// Light emission per channel.
pub const EMISSION: [f32; 3] = [10.0, 8.5, 6.0];
/// No path carries more radiance than the light emits; outputs above
/// `spp × MAX_RADIANCE` are invalid.
pub const MAX_RADIANCE: f32 = 16.0;

const EPS: f32 = 1e-4;
/// Half the vertical field of view.
const TAN_HALF_FOV: f32 = 0.33;
const CAMERA: Vec3 = Vec3::new(0.0, 0.9, 3.6);
/// The room extends to just in front of the camera, so wide frames see side walls
/// rather than the outside.
const BOX_FRONT: f32 = 3.5;
const LIGHT_HALF: f32 = 0.3;
const RED: Vec3 = Vec3::new(0.63, 0.065, 0.05);
const GREEN: Vec3 = Vec3::new(0.14, 0.45, 0.091);
const WHITE: Vec3 = Vec3::new(0.725, 0.71, 0.68);
const MIRROR_TINT: f32 = 0.95;
const GLASS_IOR: f32 = 1.5;

struct Sphere {
    centre: Vec3,
    radius: f32,
    material: Material,
}

const SPHERES: [Sphere; 2] = [
    Sphere {
        centre: Vec3::new(-0.42, 0.4, -0.3),
        radius: 0.4,
        material: Material::Mirror,
    },
    Sphere {
        centre: Vec3::new(0.45, 0.33, 0.35),
        radius: 0.33,
        material: Material::Glass,
    },
];

/// Parameters of one tile render: the tile `(x, y, w, h)` of a `width × height`
/// image, `spp` samples per pixel starting at sample `pass × spp`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileParams {
    pub width: u32,
    pub height: u32,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub spp: u32,
    pub pass: u32,
    pub seed: u32,
}

/// Renders the tile into `out`: `w × h × 3` radiance sums, row-major RGB.
pub fn render_tile(p: &TileParams, out: &mut [f32]) {
    render_rows(p, 0, p.h, out);
}

/// Renders tile rows `row_start..row_end` into `out` (`(row_end − row_start) × w × 3`
/// values). Pixels are independent, so splitting a tile into rows gives identical sums.
pub fn render_rows(p: &TileParams, row_start: u32, row_end: u32, out: &mut [f32]) {
    let rows = (row_end - row_start) as usize;
    assert_eq!(out.len(), rows * p.w as usize * 3, "output buffer size");
    for (i, row) in (row_start..row_end).enumerate() {
        for col in 0..p.w {
            let rgb = render_pixel(p, p.x + col, p.y + row);
            let at = (i * p.w as usize + col as usize) * 3;
            out[at..at + 3].copy_from_slice(&[rgb.x, rgb.y, rgb.z]);
        }
    }
}

/// Sum of radiance over the pixel's samples `pass × spp .. (pass + 1) × spp`.
pub fn render_pixel(p: &TileParams, px: u32, py: u32) -> Vec3 {
    let pixel = py.wrapping_mul(p.width).wrapping_add(px);
    let mut sum = Vec3::ZERO;
    for k in 0..p.spp {
        let sample = p.pass.wrapping_mul(p.spp).wrapping_add(k);
        let mut rng = Rng::new(p.seed, pixel, sample);
        sum = sum.add(trace_camera_ray(p, px, py, &mut rng));
    }
    sum
}

fn trace_camera_ray(p: &TileParams, px: u32, py: u32, rng: &mut Rng) -> Vec3 {
    // Vertical field of view; wider frames see more of the side walls.
    let scale = TAN_HALF_FOV / p.height as f32;
    let jx = rng.next_f32();
    let jy = rng.next_f32();
    let u = (2.0 * (px as f32 + jx) - p.width as f32) * scale;
    let v = (p.height as f32 - 2.0 * (py as f32 + jy)) * scale;
    trace(CAMERA, Vec3::new(u, v, -1.0).normalize(), rng)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Material {
    Diffuse,
    Light,
    Mirror,
    Glass,
}

struct Hit {
    t: f32,
    /// Unit normal facing the incoming ray (for spheres: outward).
    normal: Vec3,
    albedo: Vec3,
    material: Material,
}

fn trace(mut origin: Vec3, mut dir: Vec3, rng: &mut Rng) -> Vec3 {
    let mut throughput = Vec3::new(1.0, 1.0, 1.0);
    let mut radiance = Vec3::ZERO;
    for _ in 0..MAX_DEPTH {
        let Some(hit) = intersect(origin, dir) else {
            break;
        };
        let point = origin.add(dir.scale(hit.t));
        match hit.material {
            Material::Light => {
                radiance = radiance.add(throughput.mul(Vec3::from(EMISSION)));
                break;
            }
            Material::Diffuse => {
                throughput = throughput.mul(hit.albedo);
                let bounced = hit.normal.add(random_unit_vector(rng));
                let len2 = bounced.dot(bounced);
                dir = if len2 > 1e-12 {
                    bounced.scale(1.0 / len2.sqrt())
                } else {
                    hit.normal
                };
                origin = point.add(hit.normal.scale(EPS));
            }
            Material::Mirror => {
                throughput = throughput.scale(MIRROR_TINT);
                dir = reflect(dir, hit.normal);
                origin = point.add(hit.normal.scale(EPS));
            }
            Material::Glass => {
                // `hit.normal` is outward; flip it when leaving the sphere.
                let cos_out = -dir.dot(hit.normal);
                let entering = cos_out > 0.0;
                let (normal, eta) = if entering {
                    (hit.normal, 1.0 / GLASS_IOR)
                } else {
                    (hit.normal.scale(-1.0), GLASS_IOR)
                };
                let cos_i = -dir.dot(normal);
                let k = 1.0 - eta * eta * (1.0 - cos_i * cos_i);
                let r0 = (1.0 - GLASS_IOR) / (1.0 + GLASS_IOR);
                let r0 = r0 * r0;
                let m = 1.0 - cos_i;
                let fresnel = r0 + (1.0 - r0) * (m * m * m * m * m);
                if k < 0.0 || rng.next_f32() < fresnel {
                    dir = reflect(dir, normal);
                    origin = point.add(normal.scale(EPS));
                } else {
                    dir = dir
                        .scale(eta)
                        .add(normal.scale(eta * cos_i - k.sqrt()))
                        .normalize();
                    origin = point.sub(normal.scale(EPS));
                }
            }
        }
    }
    radiance
}

fn reflect(d: Vec3, n: Vec3) -> Vec3 {
    d.sub(n.scale(2.0 * d.dot(n)))
}

/// Uniform point on the unit sphere by rejection from the cube: no trigonometry, so
/// every backend draws the same direction from the same random numbers.
fn random_unit_vector(rng: &mut Rng) -> Vec3 {
    let mut v = Vec3::new(0.0, 1.0, 0.0);
    for _ in 0..16 {
        let c = Vec3::new(
            rng.next_f32() * 2.0 - 1.0,
            rng.next_f32() * 2.0 - 1.0,
            rng.next_f32() * 2.0 - 1.0,
        );
        let len2 = c.dot(c);
        if len2 > 1e-6 && len2 <= 1.0 {
            v = c.scale(1.0 / len2.sqrt());
            break;
        }
    }
    v
}

fn intersect(o: Vec3, d: Vec3) -> Option<Hit> {
    let mut best: Option<Hit> = None;
    let mut closest = f32::INFINITY;

    for s in &SPHERES {
        let oc = o.sub(s.centre);
        let b = oc.dot(d);
        let c = oc.dot(oc) - s.radius * s.radius;
        let disc = b * b - c;
        if disc <= 0.0 {
            continue;
        }
        let root = disc.sqrt();
        let mut t = -b - root;
        if t <= EPS {
            t = -b + root;
        }
        if t > EPS && t < closest {
            closest = t;
            let normal = o.add(d.scale(t)).sub(s.centre).scale(1.0 / s.radius);
            best = Some(Hit {
                t,
                normal,
                albedo: WHITE,
                material: s.material,
            });
        }
    }

    // Walls: axis-aligned quads bounding the box; the side facing the camera is open.
    let walls: [(usize, f32, Vec3); 5] = [
        (0, -1.0, RED),
        (0, 1.0, GREEN),
        (1, 0.0, WHITE),
        (1, 2.0, WHITE),
        (2, -1.0, WHITE),
    ];
    for (axis, plane, albedo) in walls {
        let da = d.axis(axis);
        if da == 0.0 {
            continue;
        }
        let t = (plane - o.axis(axis)) / da;
        if t <= EPS || t >= closest {
            continue;
        }
        let p = o.add(d.scale(t));
        if !in_box_face(p, axis) {
            continue;
        }
        closest = t;
        let mut normal = Vec3::ZERO.with_axis(axis, if da > 0.0 { -1.0 } else { 1.0 });
        let is_light =
            axis == 1 && plane == 2.0 && p.x.abs() < LIGHT_HALF && p.z.abs() < LIGHT_HALF;
        if is_light && da < 0.0 {
            // The light only emits downwards; its top side is an ordinary white face.
            normal = Vec3::new(0.0, 1.0, 0.0);
        }
        best = Some(Hit {
            t,
            normal,
            albedo,
            material: if is_light && da > 0.0 {
                Material::Light
            } else {
                Material::Diffuse
            },
        });
    }
    best
}

fn in_box_face(p: Vec3, axis: usize) -> bool {
    const SLACK: f32 = 1e-4;
    let within = |v: f32, lo: f32, hi: f32| v >= lo - SLACK && v <= hi + SLACK;
    (axis == 0 || within(p.x, -1.0, 1.0))
        && (axis == 1 || within(p.y, 0.0, 2.0))
        && (axis == 2 || within(p.z, -1.0, BOX_FRONT))
}

/// Per-sample random numbers: a pure function of `(seed, pixel, sample)` and the draw
/// count, identical in the WGSL kernel.
struct Rng(u32);

impl Rng {
    fn new(seed: u32, pixel: u32, sample: u32) -> Self {
        Self(pcg_hash(seed ^ pcg_hash(pixel ^ pcg_hash(sample))))
    }

    fn next_f32(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x9e37_79b9);
        // 24 random bits map exactly onto f32's mantissa: uniform in [0, 1).
        (lowbias32(self.0) >> 8) as f32 * (1.0 / 16_777_216.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);

    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
    fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
    fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
    fn mul(self, o: Self) -> Self {
        Self::new(self.x * o.x, self.y * o.y, self.z * o.z)
    }
    fn scale(self, s: f32) -> Self {
        Self::new(self.x * s, self.y * s, self.z * s)
    }
    fn dot(self, o: Self) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    fn normalize(self) -> Self {
        self.scale(1.0 / self.dot(self).sqrt())
    }
    fn axis(self, a: usize) -> f32 {
        [self.x, self.y, self.z][a]
    }
    fn with_axis(mut self, a: usize, v: f32) -> Self {
        match a {
            0 => self.x = v,
            1 => self.y = v,
            _ => self.z = v,
        }
        self
    }
}

impl From<[f32; 3]> for Vec3 {
    fn from([x, y, z]: [f32; 3]) -> Self {
        Self::new(x, y, z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tile(w: u32, h: u32, spp: u32, pass: u32, seed: u32) -> TileParams {
        TileParams {
            width: 64,
            height: 48,
            x: 16,
            y: 8,
            w,
            h,
            spp,
            pass,
            seed,
        }
    }

    fn render(p: &TileParams) -> Vec<f32> {
        let mut out = vec![0.0; (p.w * p.h * 3) as usize];
        render_tile(p, &mut out);
        out
    }

    /// Test vectors shared with the browser self-test. They were produced by the
    /// WebAssembly build, so this also checks that native and WebAssembly agree
    /// bit for bit.
    #[test]
    fn matches_shared_vectors() {
        #[derive(serde::Deserialize)]
        struct Vector {
            width: u32,
            height: u32,
            x: u32,
            y: u32,
            w: u32,
            h: u32,
            spp: u32,
            pass: u32,
            seed: u32,
            rgb: Vec<f32>,
        }
        let vectors: Vec<Vector> =
            serde_json::from_str(include_str!("../../../templates/pt/vectors.json")).unwrap();
        assert!(!vectors.is_empty());
        for v in vectors {
            let p = TileParams {
                width: v.width,
                height: v.height,
                x: v.x,
                y: v.y,
                w: v.w,
                h: v.h,
                spp: v.spp,
                pass: v.pass,
                seed: v.seed,
            };
            assert_eq!(render(&p), v.rgb, "{p:?}");
        }
    }

    #[test]
    fn deterministic() {
        let p = tile(8, 8, 4, 0, 7);
        assert_eq!(render(&p), render(&p));
    }

    #[test]
    fn rows_split_gives_identical_pixels() {
        let p = tile(8, 6, 3, 1, 5);
        let whole = render(&p);
        let mut top = vec![0.0; 8 * 2 * 3];
        let mut bottom = vec![0.0; 8 * 4 * 3];
        render_rows(&p, 0, 2, &mut top);
        render_rows(&p, 2, 6, &mut bottom);
        top.extend(bottom);
        assert_eq!(top, whole);
    }

    #[test]
    fn a_pixel_does_not_depend_on_its_tile() {
        let big = tile(8, 8, 2, 3, 11);
        let small = TileParams {
            x: big.x + 3,
            y: big.y + 5,
            w: 1,
            h: 1,
            ..big
        };
        let at = ((5 * 8 + 3) * 3) as usize;
        assert_eq!(render(&small), render(&big)[at..at + 3]);
    }

    #[test]
    fn looking_straight_at_the_light_returns_its_emission() {
        // In a 1 × 64 image, row 1 spans ray slopes 0.309–0.320 (v = 0.33 at the
        // top). From the camera at y = 0.9, z = 3.6 those reach the ceiling at
        // z ∈ (−0.3, 0.3): inside the light, past no sphere.
        let p = TileParams {
            width: 1,
            height: 64,
            x: 0,
            y: 0,
            w: 1,
            h: 1,
            spp: 8,
            pass: 0,
            seed: 1,
        };
        let rgb = render_pixel(&p, 0, 1);
        // Every sample of this pixel hits the light directly (no bounces).
        assert_eq!([rgb.x, rgb.y, rgb.z], EMISSION.map(|e| e * 8.0));
    }

    #[test]
    fn radiance_is_bounded_and_the_image_is_lit() {
        let p = TileParams {
            width: 32,
            height: 24,
            x: 0,
            y: 0,
            w: 32,
            h: 24,
            spp: 4,
            pass: 0,
            seed: 3,
        };
        let out = render(&p);
        assert!(out.iter().all(|v| v.is_finite() && *v >= 0.0));
        assert!(out.iter().all(|v| *v <= p.spp as f32 * MAX_RADIANCE));
        let mean = out.iter().sum::<f32>() / out.len() as f32 / p.spp as f32;
        assert!(mean > 0.05 && mean < 2.0, "mean radiance {mean}");
    }

    #[test]
    fn image_mean_is_stable_across_seeds() {
        let mean = |seed| {
            let p = TileParams {
                width: 24,
                height: 18,
                x: 0,
                y: 0,
                w: 24,
                h: 18,
                spp: 256,
                pass: 0,
                seed,
            };
            let out = render(&p);
            out.iter().sum::<f32>() / out.len() as f32 / p.spp as f32
        };
        let (a, b) = (mean(1), mean(2));
        assert!((a - b).abs() / a < 0.05, "means {a} and {b}");
    }
}
