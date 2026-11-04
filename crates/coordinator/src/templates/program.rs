//! A group's own WebAssembly program (spec 0008). The coordinator never runs it: units
//! carry input bytes, outputs come back as bytes and are kept in unit order for the
//! owner to download.

use super::Template;
use base64::{Engine, engine::general_purpose::STANDARD};
use protocol::templates::program::{
    ID, JobInfo, JobParams, JobResult, MAX_JOB_INPUT_BYTES, MAX_UNITS, UnitOutput, UnitParams,
    VERSION,
};
use serde_json::Value;

pub struct Program;

fn parse<T: serde::de::DeserializeOwned>(v: &Value, what: &str) -> Result<T, String> {
    serde_json::from_value(v.clone()).map_err(|e| format!("invalid {what}: {e}"))
}

fn to_value<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).expect("plain structs always serialize")
}

fn decode(text: &str, what: &str) -> Result<Vec<u8>, String> {
    STANDARD
        .decode(text)
        .map_err(|e| format!("{what} is not base64: {e}"))
}

/// The unit inputs of a job, decoded, in unit order.
fn inputs(p: &JobParams) -> Result<Vec<Vec<u8>>, String> {
    let inputs = match (&p.inputs, p.count) {
        (Some(_), Some(_)) => return Err("give either inputs or count, not both".into()),
        (None, None) => return Err("give inputs (one per unit) or a count of units".into()),
        (Some(inputs), None) => {
            if p.args.is_some() {
                return Err("args go with count; put them in each input instead".into());
            }
            if inputs.len() > MAX_UNITS as usize {
                return Err(format!("at most {MAX_UNITS} units"));
            }
            inputs
                .iter()
                .enumerate()
                .map(|(i, text)| decode(text, &format!("input {i}")))
                .collect::<Result<Vec<_>, _>>()?
        }
        (None, Some(count)) => {
            if count > MAX_UNITS {
                return Err(format!("at most {MAX_UNITS} units"));
            }
            let args = decode(p.args.as_deref().unwrap_or(""), "args")?;
            if 4 + args.len() > programs::MAX_INPUT_BYTES {
                return Err(format!(
                    "args may be at most {} bytes",
                    programs::MAX_INPUT_BYTES - 4
                ));
            }
            if (4 + args.len()).saturating_mul(count as usize) > MAX_JOB_INPUT_BYTES {
                return Err(format!(
                    "the units' inputs add up to more than {MAX_JOB_INPUT_BYTES} bytes"
                ));
            }
            (0..count)
                .map(|i| {
                    let mut input = i.to_le_bytes().to_vec();
                    input.extend_from_slice(&args);
                    input
                })
                .collect()
        }
    };
    if inputs.is_empty() {
        return Err("a job needs at least one unit".into());
    }
    if let Some(i) = inputs
        .iter()
        .position(|input| input.len() > programs::MAX_INPUT_BYTES)
    {
        return Err(format!(
            "input {i} is larger than {} bytes",
            programs::MAX_INPUT_BYTES
        ));
    }
    if inputs.iter().map(Vec::len).sum::<usize>() > MAX_JOB_INPUT_BYTES {
        return Err(format!(
            "the inputs add up to more than {MAX_JOB_INPUT_BYTES} bytes"
        ));
    }
    Ok(inputs)
}

impl Template for Program {
    fn id(&self) -> &'static str {
        ID
    }

    fn version(&self) -> u32 {
        VERSION
    }

    /// None: the code is the job's module, pinned by its hash in every assignment.
    fn kernel_source(&self) -> Option<&'static str> {
        None
    }

    fn split(&self, params: &Value) -> Result<Vec<Value>, String> {
        let p: JobParams = parse(params, "params")?;
        Ok(inputs(&p)?
            .iter()
            .map(|input| {
                to_value(&UnitParams {
                    input: STANDARD.encode(input),
                })
            })
            .collect())
    }

    fn validate_output(&self, _unit_params: &Value, output: &Value) -> Result<(), String> {
        let o: UnitOutput = parse(output, "output")?;
        let bytes = decode(&o.output, "output")?;
        if bytes.len() > programs::MAX_OUTPUT_BYTES {
            return Err(format!(
                "output of {} bytes exceeds {}",
                bytes.len(),
                programs::MAX_OUTPUT_BYTES
            ));
        }
        Ok(())
    }

    /// A program without imports is deterministic: every honest device returns the same
    /// bytes. Compared decoded, so a different but valid base64 spelling still agrees.
    fn agree(&self, _unit_params: &Value, a: &Value, b: &Value) -> bool {
        let bytes = |v: &Value| {
            parse::<UnitOutput>(v, "output")
                .ok()
                .and_then(|o| decode(&o.output, "output").ok())
        };
        matches!((bytes(a), bytes(b)), (Some(a), Some(b)) if a == b)
    }

    fn aggregate(&self, job_params: &Value, outputs: &[Value]) -> Result<Value, String> {
        let info: JobInfo = parse(job_params, "params")?;
        if outputs.len() != info.units as usize {
            return Err(format!(
                "expected {} outputs, got {}",
                info.units,
                outputs.len()
            ));
        }
        let mut bytes = 0u64;
        for o in outputs {
            let o: UnitOutput = parse(o, "output")?;
            bytes += decode(&o.output, "output")?.len() as u64;
        }
        Ok(to_value(&JobResult {
            outputs: info.units,
            bytes,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const PROGRAM: &str = "0190d3a2-6a3c-7c4e-9a1e-3f5c2b8d7e61";

    fn unit_inputs(params: Value) -> Result<Vec<Vec<u8>>, String> {
        Program.split(&params).map(|units| {
            units
                .iter()
                .map(|u| {
                    let u: UnitParams = serde_json::from_value(u.clone()).unwrap();
                    STANDARD.decode(u.input).unwrap()
                })
                .collect()
        })
    }

    #[test]
    fn inputs_are_given_or_counted() {
        assert_eq!(
            unit_inputs(json!({"program": PROGRAM, "inputs": ["AQI=", ""]})).unwrap(),
            vec![vec![1, 2], vec![]]
        );
        assert_eq!(
            unit_inputs(json!({"program": PROGRAM, "count": 3, "args": "/w=="})).unwrap(),
            vec![
                vec![0, 0, 0, 0, 255],
                vec![1, 0, 0, 0, 255],
                vec![2, 0, 0, 0, 255]
            ]
        );
        assert_eq!(
            unit_inputs(json!({"program": PROGRAM, "count": 2})).unwrap(),
            vec![vec![0, 0, 0, 0], vec![1, 0, 0, 0]]
        );
    }

    #[test]
    fn bad_params_are_refused() {
        let big = STANDARD.encode(vec![0u8; programs::MAX_INPUT_BYTES + 1]);
        let near = STANDARD.encode(vec![0u8; programs::MAX_INPUT_BYTES - 4]);
        for bad in [
            json!({"program": PROGRAM}),
            json!({"program": PROGRAM, "count": 0}),
            json!({"program": PROGRAM, "inputs": []}),
            json!({"program": PROGRAM, "count": 1, "inputs": [""]}),
            json!({"program": PROGRAM, "inputs": [""], "args": ""}),
            json!({"program": PROGRAM, "count": MAX_UNITS + 1}),
            json!({"program": PROGRAM, "inputs": ["not base64!"]}),
            json!({"program": PROGRAM, "count": 1, "args": "not base64!"}),
            json!({"program": PROGRAM, "inputs": [big]}),
            // 65 units × 256 KiB is over the job's 16 MiB.
            json!({"program": PROGRAM, "count": 65, "args": near}),
            json!({"program": "not a uuid", "count": 1}),
            json!({"program": PROGRAM, "count": 1, "code": "x"}),
        ] {
            assert!(Program.split(&bad).is_err(), "{bad}");
        }
        let at_limit = json!({"program": PROGRAM, "count": 64, "args": near});
        assert_eq!(Program.split(&at_limit).unwrap().len(), 64);
    }

    #[test]
    fn outputs_are_bounded_bytes() {
        let unit = json!({"input": ""});
        assert!(
            Program
                .validate_output(&unit, &json!({"output": "AQID"}))
                .is_ok()
        );
        assert!(
            Program
                .validate_output(&unit, &json!({"output": ""}))
                .is_ok()
        );
        let big = STANDARD.encode(vec![0u8; programs::MAX_OUTPUT_BYTES + 1]);
        for bad in [
            json!({"output": big}),
            json!({"output": "%%"}),
            json!({"output": 3}),
            json!({"inside": 3}),
            json!({"output": "", "extra": 1}),
        ] {
            assert!(Program.validate_output(&unit, &bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn agreement_is_byte_equality() {
        let unit = json!({"input": ""});
        let (a, b) = (json!({"output": "AQID"}), json!({"output": "AQIE"}));
        assert!(Program.agree(&unit, &a, &a));
        assert!(!Program.agree(&unit, &a, &b));
        assert!(!Program.agree(&unit, &a, &json!({"output": "%%"})));
        assert!(!Program.agree(&unit, &json!({"inside": 1}), &json!({"inside": 1})));
    }

    #[test]
    fn aggregate_counts_outputs() {
        let info = json!({"program": PROGRAM, "name": "x", "units": 2});
        let outputs = [json!({"output": "AQID"}), json!({"output": ""})];
        assert_eq!(
            Program.aggregate(&info, &outputs).unwrap(),
            json!({"outputs": 2, "bytes": 3})
        );
        assert!(Program.aggregate(&info, &outputs[..1]).is_err());
    }
}
