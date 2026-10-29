use super::Template;
use kernels::pt::MAX_RADIANCE;
use protocol::templates::pt::{
    ID, JobParams, JobResult, MAX_PASSES, MAX_SIZE, MAX_SPP, MAX_TILE, MAX_UNITS, MIN_SIZE,
    MIN_TILE, UnitOutput, UnitParams, VERSION, decode_rgb,
};
use serde_json::Value;

pub struct Pt;

fn parse<T: serde::de::DeserializeOwned>(v: &Value, what: &str) -> Result<T, String> {
    serde_json::from_value(v.clone()).map_err(|e| format!("invalid {what}: {e}"))
}

fn to_value<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).expect("plain structs always serialize")
}

fn check(name: &str, value: u32, min: u32, max: u32) -> Result<(), String> {
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(format!("{name} must be between {min} and {max}"))
    }
}

/// Decodes and checks a unit's radiance sums; shared by validation and aggregation.
pub fn unit_rgb(unit: &UnitParams, output: &Value) -> Result<Vec<f32>, String> {
    let o: UnitOutput = parse(output, "output")?;
    let rgb = decode_rgb(&o.rgb)?;
    let expected = unit.w as usize * unit.h as usize * 3;
    if rgb.len() != expected {
        return Err(format!("rgb has {} values, expected {expected}", rgb.len()));
    }
    let max = unit.spp as f32 * MAX_RADIANCE;
    if let Some(v) = rgb
        .iter()
        .find(|v| !(v.is_finite() && **v >= 0.0 && **v <= max))
    {
        return Err(format!("radiance sum {v} outside 0..={max}"));
    }
    Ok(rgb)
}

/// Whether two tiles are the same render. CPU backends agree bit for bit; GPUs may fuse
/// or approximate floating-point operations, so now and then one sample takes another
/// branch. Same rule as the browser self-test: at least 90 % of values within 0.1 % (or
/// 0.001), and the totals within 5 %.
pub fn tiles_agree(a: &[f32], b: &[f32]) -> bool {
    if a.len() != b.len() || a.is_empty() {
        return false;
    }
    if a == b {
        return true;
    }
    let close = a
        .iter()
        .zip(b)
        .filter(|(x, y)| (*x - *y).abs() <= 1e-3f32.max(1e-3 * y.abs()))
        .count();
    let (sum_a, sum_b): (f64, f64) = a.iter().zip(b).fold((0.0, 0.0), |(sa, sb), (x, y)| {
        (sa + f64::from(*x), sb + f64::from(*y))
    });
    let sum_error = if sum_b == 0.0 {
        sum_a.abs()
    } else {
        (sum_a - sum_b).abs() / sum_b
    };
    close * 10 >= a.len() * 9 && sum_error <= 0.05
}

impl Template for Pt {
    fn id(&self) -> &'static str {
        ID
    }

    fn version(&self) -> u32 {
        VERSION
    }

    fn kernel_source(&self) -> &'static str {
        include_str!("../../../../templates/pt/pt.wgsl")
    }

    /// One unit per tile per pass, pass-major: the whole image appears first, then
    /// every pass refines it.
    fn split(&self, params: &Value) -> Result<Vec<Value>, String> {
        let p: JobParams = parse(params, "params")?;
        check("width", p.width, MIN_SIZE, MAX_SIZE)?;
        check("height", p.height, MIN_SIZE, MAX_SIZE)?;
        check("tile", p.tile, MIN_TILE, MAX_TILE)?;
        check("spp", p.spp, 1, MAX_SPP)?;
        check("passes", p.passes, 1, MAX_PASSES)?;
        let tiles = p.width.div_ceil(p.tile) * p.height.div_ceil(p.tile);
        let units = tiles * p.passes;
        check("units (tiles × passes)", units, 1, MAX_UNITS)?;

        let mut out = Vec::with_capacity(units as usize);
        for pass in 0..p.passes {
            for y in (0..p.height).step_by(p.tile as usize) {
                for x in (0..p.width).step_by(p.tile as usize) {
                    out.push(to_value(&UnitParams {
                        width: p.width,
                        height: p.height,
                        x,
                        y,
                        w: p.tile.min(p.width - x),
                        h: p.tile.min(p.height - y),
                        spp: p.spp,
                        pass,
                        seed: p.seed,
                    }));
                }
            }
        }
        Ok(out)
    }

    fn validate_output(&self, unit_params: &Value, output: &Value) -> Result<(), String> {
        let u: UnitParams = parse(unit_params, "unit params")?;
        unit_rgb(&u, output).map(|_| ())
    }

    fn agree(&self, unit_params: &Value, a: &Value, b: &Value) -> bool {
        let Ok(u) = parse::<UnitParams>(unit_params, "unit params") else {
            return false;
        };
        match (unit_rgb(&u, a), unit_rgb(&u, b)) {
            (Ok(a), Ok(b)) => tiles_agree(&a, &b),
            _ => false,
        }
    }

    fn aggregate(&self, job_params: &Value, outputs: &[Value]) -> Result<Value, String> {
        let p: JobParams = parse(job_params, "params")?;
        let units = self.split(job_params)?;
        if outputs.len() != units.len() {
            return Err(format!(
                "expected {} outputs, got {}",
                units.len(),
                outputs.len()
            ));
        }
        let mut total = [0.0f64; 3];
        for (unit, output) in units.iter().zip(outputs) {
            let u: UnitParams = parse(unit, "unit params")?;
            for px in unit_rgb(&u, output)?.chunks_exact(3) {
                for (t, v) in total.iter_mut().zip(px) {
                    *t += f64::from(*v);
                }
            }
        }
        let samples_per_pixel = p.spp * p.passes;
        let samples = f64::from(p.width) * f64::from(p.height) * f64::from(samples_per_pixel);
        Ok(to_value(&JobResult {
            width: p.width,
            height: p.height,
            samples_per_pixel,
            mean: total.map(|t| (t / samples) as f32),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kernels::pt::{TileParams, render_tile};
    use protocol::templates::pt::encode_rgb;
    use serde_json::json;

    fn render(unit: &Value) -> Value {
        let u: UnitParams = serde_json::from_value(unit.clone()).unwrap();
        let mut out = vec![0.0; (u.w * u.h * 3) as usize];
        render_tile(
            &TileParams {
                width: u.width,
                height: u.height,
                x: u.x,
                y: u.y,
                w: u.w,
                h: u.h,
                spp: u.spp,
                pass: u.pass,
                seed: u.seed,
            },
            &mut out,
        );
        json!({ "rgb": encode_rgb(&out) })
    }

    #[test]
    fn split_covers_the_image_pass_by_pass() {
        let units = Pt
            .split(
                &json!({"width": 40, "height": 20, "tile": 16, "spp": 2, "passes": 2, "seed": 1}),
            )
            .unwrap();
        // 3 × 2 tiles, two passes, pass-major.
        assert_eq!(units.len(), 12);
        let u: Vec<UnitParams> = units
            .iter()
            .map(|u| serde_json::from_value(u.clone()).unwrap())
            .collect();
        assert!(u[..6].iter().all(|u| u.pass == 0));
        assert!(u[6..].iter().all(|u| u.pass == 1));
        let area: u32 = u[..6].iter().map(|u| u.w * u.h).sum();
        assert_eq!(area, 40 * 20);
        assert_eq!((u[2].x, u[2].w, u[5].y, u[5].h), (32, 8, 16, 4));
    }

    #[test]
    fn the_largest_tile_fits_in_a_worker_message() {
        let unit = json!({"width": MAX_TILE, "height": MAX_TILE, "x": 0, "y": 0,
                          "w": MAX_TILE, "h": MAX_TILE, "spp": 1, "pass": 0, "seed": 0});
        let rgb = vec![0.5f32; (MAX_TILE * MAX_TILE * 3) as usize];
        let message = json!({
            "type": "result",
            "lease_id": uuid::Uuid::now_v7(),
            "output": { "rgb": encode_rgb(&rgb) },
            "elapsed_ms": u32::MAX,
        });
        assert!(Pt.validate_output(&unit, &message["output"]).is_ok());
        let size = message.to_string().len();
        assert!(
            size < crate::ws::MAX_WORKER_MESSAGE_BYTES * 3 / 4,
            "{size} bytes leave too little headroom under the worker message limit"
        );
    }

    #[test]
    fn split_rejects_bad_params() {
        let ok = json!({"width": 64, "height": 64, "tile": 32, "spp": 4, "passes": 2, "seed": 0});
        assert!(Pt.split(&ok).is_ok());
        for (field, value) in [
            ("width", json!(8)),
            ("height", json!(4096)),
            ("tile", json!(4)),
            ("spp", json!(0)),
            ("passes", json!(1000)),
            ("seed", json!(-1)),
        ] {
            let mut bad = ok.clone();
            bad[field] = value;
            assert!(Pt.split(&bad).is_err(), "{bad}");
        }
        let mut extra = ok.clone();
        extra["code"] = json!("x");
        assert!(Pt.split(&extra).is_err());
        // 2048² in 8-pixel tiles is far more than the unit limit.
        let huge =
            json!({"width": 2048, "height": 2048, "tile": 8, "spp": 1, "passes": 1, "seed": 0});
        assert!(Pt.split(&huge).is_err());
    }

    #[test]
    fn validate_output_checks_size_and_range() {
        let unit = json!({"width": 8, "height": 8, "x": 0, "y": 0, "w": 2, "h": 1, "spp": 2, "pass": 0, "seed": 0});
        let ok = |v: &[f32]| Pt.validate_output(&unit, &json!({ "rgb": encode_rgb(v) }));
        assert!(ok(&[0.0, 1.0, 2.0, 3.0, 4.0, 32.0]).is_ok());
        assert!(ok(&[0.0; 5]).is_err());
        assert!(ok(&[0.0, 0.0, 0.0, 0.0, 0.0, 32.5]).is_err());
        assert!(ok(&[0.0, 0.0, 0.0, 0.0, 0.0, -1.0]).is_err());
        assert!(ok(&[0.0, 0.0, 0.0, 0.0, 0.0, f32::NAN]).is_err());
        assert!(Pt.validate_output(&unit, &json!({"rgb": "%%%"})).is_err());
        assert!(Pt.validate_output(&unit, &json!({"inside": 3})).is_err());
    }

    #[test]
    fn tiles_agree_exactly_or_within_gpu_tolerance() {
        let reference: Vec<f32> = (1..=100).map(|i| i as f32).collect();
        assert!(tiles_agree(&reference, &reference));
        // Rounding everywhere and a few samples that took another branch.
        let gpu: Vec<f32> = reference
            .iter()
            .enumerate()
            .map(|(i, v)| if i < 5 { v * 1.5 } else { v * (1.0 + 1e-5) })
            .collect();
        assert!(tiles_agree(&gpu, &reference));
        // A systematically different render, nothing at all, or a wrong size.
        let brighter: Vec<f32> = reference.iter().map(|v| v * 1.1).collect();
        assert!(!tiles_agree(&brighter, &reference));
        assert!(!tiles_agree(&vec![0.0; 100], &reference));
        assert!(!tiles_agree(&reference[..99], &reference));
        assert!(!tiles_agree(&[], &[]));

        let unit = json!({"width": 8, "height": 8, "x": 0, "y": 0, "w": 2, "h": 1, "spp": 2, "pass": 0, "seed": 0});
        let out = |v: &[f32]| json!({ "rgb": encode_rgb(v) });
        let a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        assert!(Pt.agree(&unit, &out(&a), &out(&a)));
        assert!(!Pt.agree(&unit, &out(&a), &out(&[0.0; 6])));
        assert!(!Pt.agree(&unit, &out(&a), &json!({"rgb": "%%%"})));
    }

    #[test]
    fn aggregate_matches_a_single_machine_render() {
        let params =
            json!({"width": 24, "height": 16, "tile": 8, "spp": 2, "passes": 2, "seed": 5});
        let outputs: Vec<Value> = Pt.split(&params).unwrap().iter().map(render).collect();
        let result: JobResult =
            serde_json::from_value(Pt.aggregate(&params, &outputs).unwrap()).unwrap();
        assert_eq!(
            (result.width, result.height, result.samples_per_pixel),
            (24, 16, 4)
        );

        // The same image in one piece: samples 0..4 of every pixel.
        let mut whole = vec![0.0; 24 * 16 * 3];
        render_tile(
            &TileParams {
                width: 24,
                height: 16,
                x: 0,
                y: 0,
                w: 24,
                h: 16,
                spp: 4,
                pass: 0,
                seed: 5,
            },
            &mut whole,
        );
        for c in 0..3 {
            let mean: f64 = whole
                .iter()
                .skip(c)
                .step_by(3)
                .map(|v| f64::from(*v))
                .sum::<f64>()
                / (24.0 * 16.0 * 4.0);
            assert!(
                (f64::from(result.mean[c]) - mean).abs() < 1e-4,
                "channel {c}"
            );
        }
        assert!(Pt.aggregate(&params, &outputs[1..]).is_err());
    }
}
