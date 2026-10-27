use super::Template;
use protocol::templates::pi::{
    ID, JobParams, JobResult, MAX_SAMPLES_PER_UNIT, MAX_UNITS, UnitOutput, UnitParams, VERSION,
};
use serde_json::Value;

pub struct Pi;

fn parse<T: serde::de::DeserializeOwned>(v: &Value, what: &str) -> Result<T, String> {
    serde_json::from_value(v.clone()).map_err(|e| format!("invalid {what}: {e}"))
}

fn to_value<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).expect("plain structs always serialize")
}

impl Template for Pi {
    fn id(&self) -> &'static str {
        ID
    }

    fn version(&self) -> u32 {
        VERSION
    }

    fn kernel_source(&self) -> &'static str {
        include_str!("../../../../templates/pi/pi.wgsl")
    }

    fn split(&self, params: &Value) -> Result<Vec<Value>, String> {
        let p: JobParams = parse(params, "params")?;
        if !(1..=MAX_UNITS).contains(&p.units) {
            return Err(format!("units must be between 1 and {MAX_UNITS}"));
        }
        if !(1..=MAX_SAMPLES_PER_UNIT).contains(&p.samples_per_unit) {
            return Err(format!(
                "samples_per_unit must be between 1 and {MAX_SAMPLES_PER_UNIT}"
            ));
        }
        Ok((0..p.units)
            .map(|unit_index| {
                to_value(&UnitParams {
                    seed: p.seed,
                    unit_index,
                    samples: p.samples_per_unit,
                })
            })
            .collect())
    }

    fn validate_output(&self, unit_params: &Value, output: &Value) -> Result<(), String> {
        let u: UnitParams = parse(unit_params, "unit params")?;
        let o: UnitOutput = parse(output, "output")?;
        if o.inside > u.samples {
            return Err(format!("inside {} exceeds samples {}", o.inside, u.samples));
        }
        Ok(())
    }

    fn aggregate(&self, job_params: &Value, outputs: &[Value]) -> Result<Value, String> {
        let p: JobParams = parse(job_params, "params")?;
        if outputs.len() != p.units as usize {
            return Err(format!(
                "expected {} outputs, got {}",
                p.units,
                outputs.len()
            ));
        }
        let mut inside = 0u64;
        for o in outputs {
            inside += u64::from(parse::<UnitOutput>(o, "output")?.inside);
        }
        let total = u64::from(p.units) * u64::from(p.samples_per_unit);
        Ok(to_value(&JobResult {
            inside,
            total,
            pi: kernels::pi::estimate(inside, total),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn split_produces_one_unit_per_index() {
        let units = Pi
            .split(&json!({"units": 3, "samples_per_unit": 100, "seed": 9}))
            .unwrap();
        assert_eq!(
            units,
            vec![
                json!({"seed": 9, "unit_index": 0, "samples": 100}),
                json!({"seed": 9, "unit_index": 1, "samples": 100}),
                json!({"seed": 9, "unit_index": 2, "samples": 100}),
            ]
        );
    }

    #[test]
    fn split_rejects_bad_params() {
        for bad in [
            json!({"units": 0, "samples_per_unit": 1, "seed": 0}),
            json!({"units": MAX_UNITS + 1, "samples_per_unit": 1, "seed": 0}),
            json!({"units": 1, "samples_per_unit": 0, "seed": 0}),
            json!({"units": 1, "samples_per_unit": MAX_SAMPLES_PER_UNIT + 1, "seed": 0}),
            json!({"units": 1, "samples_per_unit": 1}),
            json!({"units": 1, "samples_per_unit": 1, "seed": 0, "code": "x"}),
            json!({"units": -1, "samples_per_unit": 1, "seed": 0}),
        ] {
            assert!(Pi.split(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn validate_output_bounds() {
        let unit = json!({"seed": 1, "unit_index": 0, "samples": 10});
        assert!(Pi.validate_output(&unit, &json!({"inside": 10})).is_ok());
        assert!(Pi.validate_output(&unit, &json!({"inside": 11})).is_err());
        assert!(Pi.validate_output(&unit, &json!({"inside": -1})).is_err());
        assert!(
            Pi.validate_output(&unit, &json!({"inside": 1, "x": 0}))
                .is_err()
        );
    }

    #[test]
    fn aggregate_matches_single_machine_run() {
        let params = json!({"units": 8, "samples_per_unit": 50_000, "seed": 3});
        let outputs: Vec<Value> = Pi
            .split(&params)
            .unwrap()
            .iter()
            .map(|u| {
                let u: UnitParams = serde_json::from_value(u.clone()).unwrap();
                let inside = kernels::pi::count_inside(kernels::pi::UnitParams {
                    seed: u.seed,
                    unit_index: u.unit_index,
                    samples: u.samples,
                });
                json!({ "inside": inside })
            })
            .collect();
        let result: JobResult =
            serde_json::from_value(Pi.aggregate(&params, &outputs).unwrap()).unwrap();
        assert_eq!(result.total, 400_000);
        assert!((result.pi - std::f64::consts::PI).abs() < 0.02);
    }
}
