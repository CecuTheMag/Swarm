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
