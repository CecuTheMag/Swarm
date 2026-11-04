//! Neural network sweep (spec 0011): one unit trains one configuration with one seed.

use super::Template;
use kernels::nn;
use protocol::templates::nn::{
    CURVE_POINTS, DATASETS, ID, JobParams, JobResult, MAX_SEEDS, MAX_UNITS, MAX_VALUES, Row,
    UnitOutput, UnitParams, VERSION,
};
use serde_json::Value;
pub use sim_worker::network::{dataset, train_unit};

pub struct Nn;

fn parse<T: serde::de::DeserializeOwned>(v: &Value, what: &str) -> Result<T, String> {
    serde_json::from_value(v.clone()).map_err(|e| format!("invalid {what}: {e}"))
}

fn to_value<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).expect("plain structs always serialize")
}

/// Validation images per run of the dataset.
fn validation_size(name: &str) -> Result<u32, String> {
    Ok(dataset(name)?.val_y.len() as u32)
}

/// The largest loss a curve point can hold: −ln(2⁻¹⁵) in Q16.
const MAX_LOSS: u32 = 681_391;

fn check_list<T: PartialEq>(name: &str, values: &[T]) -> Result<(), String> {
    if values.is_empty() || values.len() > MAX_VALUES {
        return Err(format!("{name} needs 1 to {MAX_VALUES} values"));
    }
    for (i, v) in values.iter().enumerate() {
        if values[..i].contains(v) {
            return Err(format!("{name} lists a value twice"));
        }
    }
    Ok(())
}

/// The configurations of a sweep, in order, before seeds, each with the learning rate
/// as the person asked for it (devices get it rounded to Q16).
fn grid(p: &JobParams) -> Result<Vec<(UnitParams, f64)>, String> {
    if !DATASETS.contains(&p.dataset.as_str()) {
        return Err(format!("dataset must be one of {}", DATASETS.join(", ")));
    }
    check_list("hidden", &p.hidden)?;
    check_list("layers", &p.layers)?;
    check_list("learning_rate", &p.learning_rate)?;
    check_list("batch", &p.batch)?;
    if !(1..=MAX_SEEDS).contains(&p.seeds) {
        return Err(format!("seeds must be between 1 and {MAX_SEEDS}"));
    }
    let mut lr_q16 = Vec::new();
    for &lr in &p.learning_rate {
        let q = (lr * 65536.0).round();
        if !(lr.is_finite() && lr > 0.0 && q >= 1.0 && q <= f64::from(nn::MAX_LR_Q16)) {
            return Err("each learning_rate must be above 0 and below 0.5".into());
        }
        lr_q16.push(q as u32);
    }
    let mut batch_log2 = Vec::new();
    for &b in &p.batch {
        if !b.is_power_of_two() || b > 1 << nn::MAX_BATCH_LOG2 {
            return Err("each batch must be a power of two up to 128".into());
        }
        batch_log2.push(b.trailing_zeros());
    }
    let count = p.hidden.len() * p.layers.len() * lr_q16.len() * batch_log2.len();
    if count * p.seeds as usize > MAX_UNITS {
        return Err(format!(
            "{count} configurations × {} seeds is more than {MAX_UNITS} units",
            p.seeds
        ));
    }

    let mut configs = Vec::with_capacity(count);
    for &hidden in &p.hidden {
        for &layers in &p.layers {
            for (&lr, &asked) in lr_q16.iter().zip(&p.learning_rate) {
                for &b in &batch_log2 {
                    let unit = UnitParams {
                        dataset: p.dataset.clone(),
                        config: configs.len() as u32,
                        hidden,
                        layers,
                        lr_q16: lr,
                        batch_log2: b,
                        steps: p.steps,
                        seed: 0,
                    };
                    sim_worker::network::config(&unit).validate()?;
                    configs.push((unit, asked));
                }
            }
        }
    }
    Ok(configs)
}

/// The leaderboard from the units decided so far (all of them, once the job is done).
pub fn leaderboard(
    p: &JobParams,
    decided: &[(UnitParams, UnitOutput)],
) -> Result<JobResult, String> {
    let validation = validation_size(&p.dataset)?;
    let configs = grid(p)?;
    let mut runs: Vec<Vec<&UnitOutput>> = vec![Vec::new(); configs.len()];
    for (u, o) in decided {
        if let Some(r) = runs.get_mut(u.config as usize) {
            r.push(o);
        }
    }
    let mut rows: Vec<Row> = configs
        .iter()
        .zip(&runs)
        .filter(|(_, r)| !r.is_empty())
        .map(|((c, asked), r)| {
            let acc: Vec<f64> = r
                .iter()
                .map(|o| f64::from(o.correct) / f64::from(validation))
                .collect();
            let curve = (0..CURVE_POINTS)
                .map(|k| {
                    r.iter()
                        .map(|o| f64::from(o.curve[k]) / 65536.0)
                        .sum::<f64>()
                        / r.len() as f64
                })
                .collect();
            Row {
                config: c.config,
                hidden: c.hidden,
                layers: c.layers,
                learning_rate: *asked,
                batch: 1 << c.batch_log2,
                runs: r.len() as u32,
                accuracy: acc.iter().sum::<f64>() / acc.len() as f64,
                min: acc.iter().copied().fold(f64::INFINITY, f64::min),
                max: acc.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                curve,
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        b.accuracy
            .total_cmp(&a.accuracy)
            .then(a.config.cmp(&b.config))
    });
    Ok(JobResult {
        dataset: p.dataset.clone(),
        validation,
        rows,
    })
}

impl Template for Nn {
    fn id(&self) -> &'static str {
        ID
    }

    fn version(&self) -> u32 {
        VERSION
    }

    fn kernel_source(&self) -> &'static str {
        include_str!("../../../../templates/nn/nn.wgsl")
    }

    /// Seed-major: every configuration's first run comes before any second run, so
    /// the whole leaderboard fills in early. The same seed index gives every
    /// configuration the same starting point (common random numbers), which makes
    /// the comparison fairer.
    fn split(&self, params: &Value) -> Result<Vec<Value>, String> {
        let p: JobParams = parse(params, "params")?;
        let configs = grid(&p)?;
        let mut out = Vec::with_capacity(configs.len() * p.seeds as usize);
        for replica in 0..p.seeds {
            let seed = kernels::rng::unit_base(p.seed, replica);
            for (c, _) in &configs {
                out.push(to_value(&UnitParams { seed, ..c.clone() }));
            }
        }
        Ok(out)
    }

    fn validate_output(&self, unit_params: &Value, output: &Value) -> Result<(), String> {
        let u: UnitParams = parse(unit_params, "unit params")?;
        let o: UnitOutput = parse(output, "output")?;
        let validation = validation_size(&u.dataset)?;
        if o.correct > validation {
            return Err(format!("correct {} exceeds {validation} images", o.correct));
        }
        if o.curve.len() != CURVE_POINTS {
            return Err(format!("curve needs {CURVE_POINTS} points"));
        }
        if o.curve.iter().any(|&v| v > MAX_LOSS) {
            return Err("curve point above the largest possible loss".into());
        }
        if o.digest.len() != 64
            || !o
                .digest
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            return Err("digest must be 64 lowercase hex digits".into());
        }
        Ok(())
    }

    /// Training is bit-exact on every device: the same weights, so the same answer.
    fn agree(&self, _unit_params: &Value, a: &Value, b: &Value) -> bool {
        match (
            parse::<UnitOutput>(a, "output"),
            parse::<UnitOutput>(b, "output"),
        ) {
            (Ok(a), Ok(b)) => a == b,
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
        let decided = units
            .iter()
            .zip(outputs)
            .map(|(u, o)| Ok((parse(u, "unit params")?, parse(o, "output")?)))
            .collect::<Result<Vec<_>, String>>()?;
        Ok(to_value(&leaderboard(&p, &decided)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn params() -> Value {
        json!({"dataset": "glyphs", "steps": 120, "hidden": [16, 32], "layers": [1],
               "learning_rate": [0.02, 0.2], "batch": [16], "seeds": 2, "seed": 5})
    }

    #[test]
    fn split_is_seed_major_over_the_grid() {
        let units: Vec<UnitParams> = Nn
            .split(&params())
            .unwrap()
            .into_iter()
            .map(|u| serde_json::from_value(u).unwrap())
            .collect();
        assert_eq!(units.len(), 8);
        assert_eq!(
            units.iter().map(|u| u.config).collect::<Vec<_>>(),
            [0, 1, 2, 3, 0, 1, 2, 3]
        );
        assert_eq!((units[1].hidden, units[1].lr_q16), (16, 13107));
        assert_eq!((units[2].hidden, units[2].lr_q16), (32, 1311));
        assert_eq!(units[0].batch_log2, 4);
        // One seed per run, shared by every configuration of that run.
        assert!(units[..4].iter().all(|u| u.seed == units[0].seed));
        assert_ne!(units[0].seed, units[4].seed);
    }

    #[test]
    fn split_refuses_bad_sweeps() {
        let with = |key: &str, v: Value| {
            let mut p = params();
            p[key] = v;
            p
        };
        for bad in [
            with("dataset", json!("cats")),
            with("hidden", json!([])),
            with("hidden", json!([16, 16])),
            with("hidden", json!([0])),
            with("layers", json!([3])),
            with("learning_rate", json!([0.5])),
            with("learning_rate", json!([0.0])),
            with("learning_rate", json!([-0.1])),
            with("batch", json!([24])),
            with("batch", json!([256])),
            with("seeds", json!(0)),
            with("seeds", json!(6)),
            with("steps", json!(0)),
            json!({"dataset": "glyphs", "steps": 5000, "hidden": [256], "layers": [2],
                   "learning_rate": [0.1], "batch": [128], "seeds": 1, "seed": 0}),
            with("code", json!("x")),
        ] {
            assert!(Nn.split(&bad).is_err(), "{bad}");
        }
        // 8 × 8 × 8 × 4 configurations are too many units.
        let mut big = params();
        big["hidden"] = json!([8, 16, 24, 32, 40, 48, 56, 64]);
        big["layers"] = json!([1, 2]);
        big["learning_rate"] = json!([0.01, 0.02, 0.05, 0.1, 0.15, 0.2, 0.3, 0.4]);
        big["batch"] = json!([1, 2, 4, 8, 16, 32, 64, 128]);
        assert!(Nn.split(&big).is_err());
    }

    #[test]
    fn devices_agree_only_on_identical_training() {
        let unit: UnitParams =
            serde_json::from_value(Nn.split(&params()).unwrap()[3].clone()).unwrap();
        let (output, _) = train_unit(&unit).unwrap();
        let (again, _) = train_unit(&unit).unwrap();
        let (u, a) = (to_value(&unit), to_value(&output));
        assert!(Nn.validate_output(&u, &a).is_ok());
        assert!(Nn.agree(&u, &a, &to_value(&again)));
        let mut lie = output.clone();
        lie.correct += 1;
        assert!(!Nn.agree(&u, &a, &to_value(&lie)));
        let mut other = output.clone();
        other.digest = "0".repeat(64);
        assert!(!Nn.agree(&u, &a, &to_value(&other)));
    }

    #[test]
    fn malformed_outputs_are_rejected() {
        let u = Nn.split(&params()).unwrap()[0].clone();
        let good = json!({"correct": 10, "curve": vec![1000; 16], "digest": "a".repeat(64)});
        assert!(Nn.validate_output(&u, &good).is_ok());
        for bad in [
            json!({"correct": 1025, "curve": vec![1000; 16], "digest": "a".repeat(64)}),
            json!({"correct": 10, "curve": vec![1000; 15], "digest": "a".repeat(64)}),
            json!({"correct": 10, "curve": vec![MAX_LOSS + 1; 16], "digest": "a".repeat(64)}),
            json!({"correct": 10, "curve": vec![1000; 16], "digest": "A".repeat(64)}),
            json!({"correct": 10, "curve": vec![1000; 16], "digest": "a".repeat(63)}),
            json!({"correct": 10, "curve": vec![1000; 16], "digest": "g".repeat(64)}),
            json!({"correct": 10, "curve": vec![1000; 16], "digest": "a".repeat(64), "w": 1}),
        ] {
            assert!(Nn.validate_output(&u, &bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_leaderboard_ranks_configurations_by_mean_accuracy() {
        let p = params();
        let units = Nn.split(&p).unwrap();
        let outputs: Vec<Value> = units
            .iter()
            .map(|u| {
                to_value(
                    &train_unit(&serde_json::from_value(u.clone()).unwrap())
                        .unwrap()
                        .0,
                )
            })
            .collect();
        let result: JobResult =
            serde_json::from_value(Nn.aggregate(&p, &outputs).unwrap()).unwrap();
        assert_eq!(result.rows.len(), 4);
        assert!(
            result
                .rows
                .windows(2)
                .all(|w| w[0].accuracy >= w[1].accuracy)
        );
        assert!(
            result
                .rows
                .iter()
                .all(|r| r.runs == 2 && r.min <= r.accuracy && r.accuracy <= r.max)
        );
        // The larger learning rate learns more in 120 steps.
        assert_eq!(result.rows[0].learning_rate, 0.2);
        assert!(Nn.aggregate(&p, &outputs[1..]).is_err());
    }
}

#[cfg(test)]
mod vectors {
    //! `templates/nn/vectors.json`: fixed configurations and the reference answers the
    //! browser kernels must reproduce bit for bit (self-test page, browser tests).
    //! `UPDATE_VECTORS=1 cargo test -p coordinator nn::vectors` rewrites the file.

    use super::*;

    #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
    struct Vector {
        #[serde(flatten)]
        params: UnitParams,
        #[serde(flatten)]
        output: UnitOutput,
    }

    const PATH: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../templates/nn/vectors.json"
    );

    fn unit(
        config: u32,
        hidden: u32,
        layers: u32,
        lr: f64,
        batch_log2: u32,
        steps: u32,
        seed: u32,
    ) -> UnitParams {
        UnitParams {
            dataset: "glyphs".into(),
            config,
            hidden,
            layers,
            lr_q16: (lr * 65536.0).round() as u32,
            batch_log2,
            steps,
            seed,
        }
    }

    fn computed() -> Vec<Vector> {
        [
            unit(0, 8, 1, 0.1, 3, 20, 1),
            unit(1, 16, 2, 0.2, 4, 30, 2),
            unit(2, 5, 2, 0.45, 0, 40, 3),
            unit(3, 32, 1, 0.05, 7, 5, 0xffff_ffff),
            UnitParams {
                dataset: "mnist".into(),
                ..unit(4, 24, 1, 0.3, 4, 40, 5)
            },
            UnitParams {
                dataset: "mnist".into(),
                ..unit(5, 12, 2, 0.4, 2, 30, 6)
            },
        ]
        .into_iter()
        .map(|params| {
            let (output, _) = train_unit(&params).unwrap();
            Vector { params, output }
        })
        .collect()
    }

    #[test]
    fn shared_vectors_match_the_reference() {
        let vectors = computed();
        if std::env::var_os("UPDATE_VECTORS").is_some() {
            std::fs::write(PATH, serde_json::to_string_pretty(&vectors).unwrap() + "\n").unwrap();
        }
        let file: Vec<Vector> =
            serde_json::from_str(&std::fs::read_to_string(PATH).unwrap()).unwrap();
        assert_eq!(file, vectors);
    }
}
