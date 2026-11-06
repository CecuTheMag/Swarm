//! School timetables (spec 0013): one unit is one search; the coordinator re-scores every
//! timetable a device returns.

use super::Template;
use base64::{Engine, engine::general_purpose::STANDARD};
use protocol::templates::tt::{
    ID, JobParams, JobResult, MAX_ITERATIONS, MAX_SEARCHES, MIN_ITERATIONS, UnitOutput, UnitParams,
    VERSION, school_bytes,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use timetable::Problem;

pub struct Tt;

/// Largest school accepted, as JSON: the largest school the limits allow fits easily.
pub const MAX_SCHOOL_BYTES: usize = 1 << 20;

fn parse<T: serde::de::DeserializeOwned>(v: &Value, what: &str) -> Result<T, String> {
    serde_json::from_value(v.clone()).map_err(|e| format!("invalid {what}: {e}"))
}

fn to_value<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).expect("plain structs always serialize")
}

/// The slots of a timetable from their base64 (one little-endian `u16` each).
pub fn decode_slots(text: &str) -> Result<Vec<u16>, String> {
    let bytes = STANDARD
        .decode(text)
        .map_err(|e| format!("slots are not base64: {e}"))?;
    if bytes.len() % 2 != 0 {
        return Err("slots are not a whole number of u16".into());
    }
    Ok(bytes
        .chunks_exact(2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .collect())
}

pub fn encode_slots(slots: &[u16]) -> String {
    STANDARD.encode(
        slots
            .iter()
            .flat_map(|s| s.to_le_bytes())
            .collect::<Vec<u8>>(),
    )
}

impl Template for Tt {
    fn id(&self) -> &'static str {
        ID
    }

    fn version(&self) -> u32 {
        VERSION
    }

    /// The solver is Rust compiled to WebAssembly, not WGSL, so the pin is its behaviour:
    /// the fixed inputs and the exact outputs it must give for them.
    fn kernel_source(&self) -> Option<&'static str> {
        Some(include_str!("../../../../templates/tt/vectors.json"))
    }

    fn split(&self, params: &Value) -> Result<Vec<Value>, String> {
        let p: JobParams = parse(params, "params")?;
        Problem::new(&p.school)?;
        if !(1..=MAX_SEARCHES).contains(&p.searches) {
            return Err(format!("searches must be between 1 and {MAX_SEARCHES}"));
        }
        if !(MIN_ITERATIONS..=MAX_ITERATIONS).contains(&p.iterations) {
            return Err(format!(
                "iterations must be between {MIN_ITERATIONS} and {MAX_ITERATIONS}"
            ));
        }
        let bytes = school_bytes(&p.school);
        if bytes.len() > MAX_SCHOOL_BYTES {
            return Err(format!(
                "the school's data is larger than {MAX_SCHOOL_BYTES} bytes"
            ));
        }
        let school = hex::encode(Sha256::digest(&bytes));
        Ok((0..p.searches)
            .map(|index| {
                to_value(&UnitParams {
                    school: school.clone(),
                    index,
                    iterations: p.iterations,
                    seed: p.seed,
                })
            })
            .collect())
    }

    fn blob(&self, params: &Value) -> Option<Vec<u8>> {
        let p: JobParams = parse(params, "params").ok()?;
        Some(school_bytes(&p.school))
    }

    fn validate_output(&self, _unit_params: &Value, output: &Value) -> Result<(), String> {
        let o: UnitOutput = parse(output, "output")?;
        decode_slots(&o.slots).map(|_| ())
    }

    /// The timetable must be one the school allows (right length, every slot in the
    /// week), and its cost must be the one claimed: a device cannot report a better
    /// timetable than the one it sends.
    fn check_output(
        &self,
        job_params: &Value,
        unit_params: &Value,
        output: &Value,
    ) -> Result<(), String> {
        self.validate_output(unit_params, output)?;
        let p: JobParams = parse(job_params, "params")?;
        let o: UnitOutput = parse(output, "output")?;
        let problem = Problem::new(&p.school)?;
        let cost = timetable::cost(&problem, &decode_slots(&o.slots)?)?;
        if cost.total != o.cost {
            return Err(format!(
                "claimed cost {} but the timetable costs {}",
                o.cost, cost.total
            ));
        }
        Ok(())
    }

    /// A search is a pure function of its unit: honest devices return the same timetable.
    fn agree(&self, _unit_params: &Value, a: &Value, b: &Value) -> bool {
        match (
            parse::<UnitOutput>(a, "output"),
            parse::<UnitOutput>(b, "output"),
        ) {
            (Ok(a), Ok(b)) => {
                a.cost == b.cost
                    && matches!((decode_slots(&a.slots), decode_slots(&b.slots)), (Ok(x), Ok(y)) if x == y)
            }
            _ => false,
        }
    }

    /// The best timetable of all searches (the earliest unit on ties), with its cost by
    /// rule, and every search's cost.
    fn aggregate(&self, job_params: &Value, outputs: &[Value]) -> Result<Value, String> {
        let p: JobParams = parse(job_params, "params")?;
        if outputs.len() != p.searches as usize {
            return Err(format!(
                "expected {} outputs, got {}",
                p.searches,
                outputs.len()
            ));
        }
        let outputs: Vec<UnitOutput> = outputs
            .iter()
            .map(|o| parse(o, "output"))
            .collect::<Result<_, _>>()?;
        let costs: Vec<u64> = outputs.iter().map(|o| o.cost).collect();
        let best = (0..outputs.len())
            .min_by_key(|&i| (costs[i], i))
            .expect("at least one search");
        let problem = Problem::new(&p.school)?;
        let cost = timetable::cost(&problem, &decode_slots(&outputs[best].slots)?)?;
        Ok(to_value(&JobResult {
            best: best as u32,
            cost,
            slots: outputs[best].slots.clone(),
            costs,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use timetable::{Search, sample, solve};

    fn job(classes: usize, searches: u32) -> Value {
        json!({
            "school": sample::school(3, classes),
            "searches": searches,
            "iterations": 20_000,
            "seed": 11,
        })
    }

    /// What an honest device returns for a unit.
    fn honest(job: &Value, unit: &Value) -> Value {
        let p: JobParams = serde_json::from_value(job.clone()).unwrap();
        let u: UnitParams = serde_json::from_value(unit.clone()).unwrap();
        let problem = Problem::new(&p.school).unwrap();
        let slots = solve(&problem, Search::unit(u.seed, u.index, u.iterations));
        let cost = timetable::cost(&problem, &slots).unwrap().total;
        json!({ "slots": encode_slots(&slots), "cost": cost })
    }

    #[test]
    fn units_refer_to_the_school_by_its_hash() {
        let params = job(4, 3);
        let units = Tt.split(&params).unwrap();
        assert_eq!(units.len(), 3);
        let blob = Tt.blob(&params).unwrap();
        let sha = hex::encode(Sha256::digest(&blob));
        for (i, u) in units.iter().enumerate() {
            assert_eq!(
                u,
                &json!({ "school": sha, "index": i, "iterations": 20_000, "seed": 11 })
            );
        }
    }

    #[test]
    fn bad_jobs_are_refused() {
        let mut bad = Vec::new();
        for (key, value) in [
            ("searches", json!(0)),
            ("searches", json!(MAX_SEARCHES + 1)),
            ("iterations", json!(MIN_ITERATIONS - 1)),
            ("iterations", json!(MAX_ITERATIONS + 1)),
        ] {
            let mut p = job(2, 2);
            p[key] = value;
            bad.push(p);
        }
        let mut no_lessons = job(2, 2);
        no_lessons["school"]["lessons"] = json!([]);
        bad.push(no_lessons);
        let mut extra = job(2, 2);
        extra["code"] = json!("x");
        bad.push(extra);
        for p in bad {
            assert!(Tt.split(&p).is_err(), "{p}");
        }
    }

    #[test]
    fn honest_timetables_pass_and_forgeries_do_not() {
        let params = job(3, 1);
        let unit = &Tt.split(&params).unwrap()[0];
        let good = honest(&params, unit);
        assert!(Tt.check_output(&params, unit, &good).is_ok());

        // A better cost than the timetable has.
        let mut lie = good.clone();
        lie["cost"] = json!(0);
        if good["cost"] != json!(0) {
            let err = Tt.check_output(&params, unit, &lie).unwrap_err();
            assert!(err.contains("claimed cost 0"), "{err}");
        }
        // A worse cost than the timetable has is a lie too.
        let mut modest = good.clone();
        modest["cost"] = json!(good["cost"].as_u64().unwrap() + 1);
        assert!(Tt.check_output(&params, unit, &modest).is_err());
        // Wrong length, a slot outside the week, malformed.
        let slots = decode_slots(good["slots"].as_str().unwrap()).unwrap();
        let short = json!({ "slots": encode_slots(&slots[1..]), "cost": good["cost"] });
        let mut wild = slots.clone();
        wild[0] = 35;
        let outside = json!({ "slots": encode_slots(&wild), "cost": good["cost"] });
        for bad in [
            short,
            outside,
            json!({ "slots": "%%", "cost": 0 }),
            json!({ "slots": "AQ==", "cost": 0 }),
            json!({ "cost": 0 }),
            json!({ "slots": "", "cost": 0, "x": 1 }),
        ] {
            assert!(Tt.check_output(&params, unit, &bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_device_that_moves_a_pinned_hour_cannot_win() {
        let mut params = job(2, 2);
        params["school"]["lessons"][0]["pinned"] = json!([{ "day": 4, "period": 6 }]);
        let units = Tt.split(&params).unwrap();
        let good = honest(&params, &units[0]);
        let mut slots = decode_slots(good["slots"].as_str().unwrap()).unwrap();
        assert_eq!(slots[0], 4 * 7 + 6, "the solver keeps the pin");
        // The pinned hour moved: a forged cost is refused, the true one is a hard
        // violation, worse than any honest timetable without one.
        slots[0] = 0;
        let p: JobParams = serde_json::from_value(params.clone()).unwrap();
        let moved = timetable::cost(&Problem::new(&p.school).unwrap(), &slots).unwrap();
        assert_eq!(moved.breakdown.pins_moved, 1);
        let forged = json!({ "slots": encode_slots(&slots), "cost": good["cost"] });
        if moved.total != good["cost"].as_u64().unwrap() {
            assert!(Tt.check_output(&params, &units[0], &forged).is_err());
        }
        let truthful = json!({ "slots": encode_slots(&slots), "cost": moved.total });
        assert!(Tt.check_output(&params, &units[0], &truthful).is_ok());
        let result: JobResult = serde_json::from_value(
            Tt.aggregate(&params, &[truthful, honest(&params, &units[1])])
                .unwrap(),
        )
        .unwrap();
        assert_eq!(result.best, 1);
        assert_eq!(result.cost.breakdown.pins_moved, 0);
    }

    #[test]
    fn agreement_is_equality() {
        let params = job(2, 2);
        let units = Tt.split(&params).unwrap();
        let (a, b) = (honest(&params, &units[0]), honest(&params, &units[1]));
        assert!(Tt.agree(&units[0], &a, &honest(&params, &units[0])));
        assert!(!Tt.agree(&units[0], &a, &b) || a == b);
        let mut other_cost = a.clone();
        other_cost["cost"] = json!(a["cost"].as_u64().unwrap() + 5);
        assert!(!Tt.agree(&units[0], &a, &other_cost));
    }

    #[test]
    fn the_best_search_wins_and_ties_go_to_the_earliest() {
        let params = job(3, 4);
        let units = Tt.split(&params).unwrap();
        let outputs: Vec<Value> = units.iter().map(|u| honest(&params, u)).collect();
        let result: JobResult =
            serde_json::from_value(Tt.aggregate(&params, &outputs).unwrap()).unwrap();
        let costs: Vec<u64> = outputs
            .iter()
            .map(|o| o["cost"].as_u64().unwrap())
            .collect();
        let min = *costs.iter().min().unwrap();
        assert_eq!(result.costs, costs);
        assert_eq!(
            result.best as usize,
            costs.iter().position(|&c| c == min).unwrap()
        );
        assert_eq!(result.cost.total, min);
        assert_eq!(result.slots, outputs[result.best as usize]["slots"]);

        let same = vec![outputs[1].clone(); 4];
        let tied: JobResult =
            serde_json::from_value(Tt.aggregate(&params, &same).unwrap()).unwrap();
        assert_eq!(tied.best, 0);
        assert!(Tt.aggregate(&params, &outputs[..3]).is_err());
    }
}

#[cfg(test)]
mod vectors {
    //! `templates/tt/vectors.json`: fixed schools and searches, and the exact timetables
    //! the solver must give for them, natively and in the browser (browser tests). Its
    //! hash pins the template. `UPDATE_VECTORS=1 cargo test -p coordinator tt::vectors`
    //! rewrites the file.

    use super::*;
    use timetable::{School, Search, sample, solve};

    #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
    struct Vector {
        school: School,
        seed: u32,
        index: u32,
        iterations: u32,
        cost: u64,
        /// Hex SHA-256 of the slots' bytes.
        slots_sha256: String,
    }

    const PATH: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../templates/tt/vectors.json"
    );

    /// An example school with the first hour of every third lesson pinned where a
    /// search put it, as a deputy director would fix a found timetable (spec 0015).
    fn pinned(school_seed: u64, classes: usize) -> School {
        let mut school = sample::school(school_seed, classes);
        let problem = Problem::new(&school).unwrap();
        let slots = solve(&problem, Search::unit(1, 0, 20_000));
        let mut first = 0;
        for (i, l) in school.lessons.iter_mut().enumerate() {
            if i % 3 == 0 {
                l.pinned = vec![problem.slot_of(usize::from(slots[first]))];
            }
            first += usize::from(l.per_week);
        }
        school
    }

    fn computed() -> Vec<Vector> {
        [
            (sample::school(1, 1), 7, 0, 20_000),
            (sample::school(2, 3), 7, 1, 50_000),
            (sample::school(3, 6), 2026, 2, 30_000),
            (sample::school(4, 6), 2026, 3, 30_000),
            (pinned(5, 4), 2026, 1, 30_000),
        ]
        .into_iter()
        .map(|(school, seed, index, iterations)| {
            let problem = Problem::new(&school).unwrap();
            let slots = solve(&problem, Search::unit(seed, index, iterations));
            let bytes: Vec<u8> = slots.iter().flat_map(|s| s.to_le_bytes()).collect();
            Vector {
                cost: timetable::cost(&problem, &slots).unwrap().total,
                slots_sha256: hex::encode(Sha256::digest(&bytes)),
                school,
                seed,
                index,
                iterations,
            }
        })
        .collect()
    }

    #[test]
    fn shared_vectors_match_the_solver() {
        let vectors = computed();
        if std::env::var_os("UPDATE_VECTORS").is_some() {
            std::fs::write(PATH, serde_json::to_string(&vectors).unwrap() + "\n").unwrap();
        }
        let file: Vec<Vector> =
            serde_json::from_str(&std::fs::read_to_string(PATH).unwrap()).unwrap();
        assert_eq!(file, vectors);
    }
}
