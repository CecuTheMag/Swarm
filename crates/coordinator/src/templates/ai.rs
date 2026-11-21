//! Language models on the swarm (spec 0017): one unit is one prompt, answered by a whole
//! model in the device's browser. Decoding is greedy, so two honest devices with the same
//! shader compiler return the same tokens; devices that round differently may part at a
//! near-tie, which the agreement allows (`NEAR_TIE`).

use super::Template;
use protocol::templates::ai::{
    Answer, CATALOG, ID, JobParams, JobResult, MAX_JOB_BYTES, MAX_NEW_TOKENS, MAX_PROMPT_BYTES,
    MAX_PROMPTS, MAX_TEXT_BYTES, Model, NEAR_TIE, UnitOutput, UnitParams, VERSION, model,
};
use serde_json::Value;

pub struct Ai;

fn parse<T: serde::de::DeserializeOwned>(v: &Value, what: &str) -> Result<T, String> {
    serde_json::from_value(v.clone()).map_err(|e| format!("invalid {what}: {e}"))
}

fn to_value<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).expect("plain structs always serialize")
}

fn check_text(what: &str, text: &str) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err(format!("{what} is empty"));
    }
    if text.len() > MAX_PROMPT_BYTES {
        return Err(format!("{what} is longer than {MAX_PROMPT_BYTES} bytes"));
    }
    Ok(())
}

/// The catalog model a job names.
pub fn job_model(params: &Value) -> Option<&'static Model> {
    params.get("model")?.as_str().and_then(model)
}

impl Template for Ai {
    fn id(&self) -> &'static str {
        ID
    }

    fn version(&self) -> u32 {
        VERSION
    }

    /// The model catalog: devices run only the models it lists, and refuse any file whose
    /// SHA-256 differs from it.
    fn kernel_source(&self) -> Option<&'static str> {
        Some(CATALOG)
    }

    fn split(&self, params: &Value) -> Result<Vec<Value>, String> {
        let p: JobParams = parse(params, "params")?;
        if model(&p.model).is_none() {
            return Err(format!("unknown model {:?}", p.model));
        }
        if p.prompts.is_empty() || p.prompts.len() > MAX_PROMPTS {
            return Err(format!("a job needs 1 to {MAX_PROMPTS} prompts"));
        }
        let mut total = 0;
        for (i, prompt) in p.prompts.iter().enumerate() {
            check_text(&format!("prompt {}", i + 1), prompt)?;
            total += prompt.len();
        }
        if total > MAX_JOB_BYTES {
            return Err(format!(
                "the prompts are longer than {MAX_JOB_BYTES} bytes together"
            ));
        }
        if let Some(system) = &p.system {
            check_text("the system prompt", system)?;
        }
        if !(1..=MAX_NEW_TOKENS).contains(&p.max_new_tokens) {
            return Err(format!(
                "max_new_tokens must be between 1 and {MAX_NEW_TOKENS}"
            ));
        }
        Ok(p.prompts
            .into_iter()
            .map(|prompt| {
                to_value(&UnitParams {
                    model: p.model.clone(),
                    prompt,
                    system: p.system.clone(),
                    max_new_tokens: p.max_new_tokens,
                })
            })
            .collect())
    }

    fn min_gpu_buffer(&self, params: &Value) -> Option<u64> {
        job_model(params)?.min_gpu_buffer
    }

    fn test_only(&self, params: &Value) -> bool {
        job_model(params).is_some_and(|m| m.test)
    }

    fn validate_output(&self, unit_params: &Value, output: &Value) -> Result<(), String> {
        let u: UnitParams = parse(unit_params, "unit params")?;
        let o: UnitOutput = parse(output, "output")?;
        if o.tokens.is_empty() || o.tokens.len() > u.max_new_tokens as usize {
            return Err(format!(
                "{} tokens; a unit generates 1 to {}",
                o.tokens.len(),
                u.max_new_tokens
            ));
        }
        if o.margins.len() != o.tokens.len() {
            return Err("one margin per token is needed".into());
        }
        if o.margins.iter().any(|m| !m.is_finite() || *m < 0.0) {
            return Err("margins must be finite and not negative".into());
        }
        if o.text.len() > MAX_TEXT_BYTES {
            return Err(format!("the text is longer than {MAX_TEXT_BYTES} bytes"));
        }
        Ok(())
    }

    /// The same tokens and the same text; or the same tokens up to one that both devices
    /// chose by less than `NEAR_TIE`, where GPUs that round differently honestly part. Not
    /// at the first token: an honest device was unsure there for 16 of 150 measured
    /// answers, so a made-up answer would pass that often, while every measured honest
    /// parting came at token 5 or later. Matching the first token takes running the model.
    fn agree(&self, _unit_params: &Value, a: &Value, b: &Value) -> bool {
        let (Ok(a), Ok(b)) = (
            parse::<UnitOutput>(a, "output"),
            parse::<UnitOutput>(b, "output"),
        ) else {
            return false;
        };
        if a.tokens == b.tokens {
            return a.text == b.text;
        }
        let Some(i) = a.tokens.iter().zip(&b.tokens).position(|(x, y)| x != y) else {
            // One is the other cut short: no near-tie to part at.
            return false;
        };
        i > 0 && a.margins[i] < NEAR_TIE && b.margins[i] < NEAR_TIE
    }

    fn aggregate(&self, job_params: &Value, outputs: &[Value]) -> Result<Value, String> {
        let p: JobParams = parse(job_params, "params")?;
        if outputs.len() != p.prompts.len() {
            return Err(format!(
                "expected {} outputs, got {}",
                p.prompts.len(),
                outputs.len()
            ));
        }
        let answers = outputs
            .iter()
            .map(|o| {
                let o: UnitOutput = parse(o, "output")?;
                Ok(Answer {
                    text: o.text,
                    tokens: o.tokens.len() as u32,
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(to_value(&JobResult {
            model: p.model,
            answers,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const TINY: &str = "hf-internal-testing/tiny-random-LlamaForCausalLM";
    const SMOL: &str = "HuggingFaceTB/SmolLM2-360M-Instruct";

    fn params(prompts: &[&str]) -> Value {
        json!({ "model": SMOL, "prompts": prompts, "system": "Answer briefly.", "max_new_tokens": 32 })
    }

    fn output(tokens: &[u32], text: &str) -> Value {
        json!({ "text": text, "tokens": tokens, "margins": vec![1.5; tokens.len()] })
    }

    #[test]
    fn the_catalog_pins_every_file() {
        let catalog = protocol::templates::ai::catalog();
        assert!(catalog.models.iter().any(|m| m.id == SMOL && !m.test));
        assert!(catalog.models.iter().any(|m| m.id == TINY && m.test));
        for m in &catalog.models {
            assert!(!m.files.is_empty());
            assert_eq!(m.revision.len(), 40, "{}", m.id);
            for f in &m.files {
                assert_eq!(f.sha256.len(), 64, "{} {}", m.id, f.path);
                assert!(f.size > 0);
                assert!(!f.path.contains(".."), "{}", f.path);
            }
        }
    }

    #[test]
    fn one_unit_per_prompt_in_order() {
        let units = Ai.split(&params(&["first", "second", "third"])).unwrap();
        assert_eq!(units.len(), 3);
        let second: UnitParams = serde_json::from_value(units[1].clone()).unwrap();
        assert_eq!(second.prompt, "second");
        assert_eq!(second.model, SMOL);
        assert_eq!(second.system.as_deref(), Some("Answer briefly."));
        assert_eq!(second.max_new_tokens, 32);
    }

    #[test]
    fn bad_jobs_are_refused() {
        let long = "x".repeat(MAX_PROMPT_BYTES + 1);
        for p in [
            json!({ "model": "someone/else", "prompts": ["hi"], "max_new_tokens": 8 }),
            json!({ "model": SMOL, "prompts": [], "max_new_tokens": 8 }),
            json!({ "model": SMOL, "prompts": ["  "], "max_new_tokens": 8 }),
            json!({ "model": SMOL, "prompts": [long], "max_new_tokens": 8 }),
            json!({ "model": SMOL, "prompts": ["hi"], "max_new_tokens": 0 }),
            json!({ "model": SMOL, "prompts": ["hi"], "max_new_tokens": MAX_NEW_TOKENS + 1 }),
            json!({ "model": SMOL, "prompts": ["hi"], "system": "", "max_new_tokens": 8 }),
            json!({ "model": SMOL, "prompts": ["hi"], "max_new_tokens": 8, "temperature": 1 }),
        ] {
            assert!(Ai.split(&p).is_err(), "{p}");
        }
    }

    #[test]
    fn the_model_decides_what_a_device_needs() {
        assert_eq!(Ai.min_gpu_buffer(&params(&["hi"])), Some(256 << 20));
        assert!(!Ai.test_only(&params(&["hi"])));
        let tiny = json!({ "model": TINY, "prompts": ["hi"], "max_new_tokens": 4 });
        assert_eq!(Ai.min_gpu_buffer(&tiny), None);
        assert!(Ai.test_only(&tiny));
    }

    #[test]
    fn outputs_are_checked() {
        let unit = &Ai.split(&params(&["hi"])).unwrap()[0];
        assert!(Ai.validate_output(unit, &output(&[1, 2, 3], "abc")).is_ok());
        assert!(Ai.validate_output(unit, &output(&[], "")).is_err());
        assert!(
            Ai.validate_output(unit, &output(&[7; 33], "too many"))
                .is_err()
        );
        let wrong_margins = json!({ "text": "a", "tokens": [1, 2], "margins": [1.0] });
        assert!(Ai.validate_output(unit, &wrong_margins).is_err());
        let negative = json!({ "text": "a", "tokens": [1], "margins": [-0.5] });
        assert!(Ai.validate_output(unit, &negative).is_err());
        let extra = json!({ "text": "a", "tokens": [1], "margins": [0.5], "logits": [] });
        assert!(Ai.validate_output(unit, &extra).is_err());
    }

    #[test]
    fn agreement_is_the_same_tokens_and_text() {
        let unit = &Ai.split(&params(&["hi"])).unwrap()[0];
        let a = output(&[5, 6, 7], "Hello");
        let mut other_margins = a.clone();
        other_margins["margins"] = json!([0.1, 9.0, 3.0]);
        assert!(Ai.agree(unit, &a, &other_margins));
        assert!(!Ai.agree(unit, &a, &output(&[5, 6, 8], "Hello")));
        // The right tokens with made-up text are not the same answer.
        assert!(!Ai.agree(unit, &a, &output(&[5, 6, 7], "Goodbye")));
    }

    fn with_margins(tokens: &[u32], margins: &[f32]) -> Value {
        json!({ "text": format!("{tokens:?}"), "tokens": tokens, "margins": margins })
    }

    #[test]
    fn answers_parting_at_an_honest_near_tie_agree() {
        let unit = &Ai.split(&params(&["hi"])).unwrap()[0];
        // The same until token 2, which both devices chose by less than the threshold.
        let a = with_margins(&[5, 6, 7, 8], &[3.0, 2.0, 0.1, 4.0]);
        let b = with_margins(&[5, 6, 9, 1], &[3.0, 2.0, 0.3, 1.0]);
        assert!(Ai.agree(unit, &a, &b));
        assert!(Ai.agree(unit, &b, &a));
        // One device was sure of its token there: not a near-tie.
        let sure = with_margins(&[5, 6, 9, 1], &[3.0, 2.0, NEAR_TIE, 1.0]);
        assert!(!Ai.agree(unit, &a, &sure));
        // A made-up answer parts at the first token, where the honest device was sure,
        // whatever margins the liar reports.
        let liar = with_margins(&[1, 2, 3, 4], &[0.0, 0.0, 0.0, 0.0]);
        assert!(!Ai.agree(unit, &a, &liar));
        // Not at the first token, even when the honest device was unsure there.
        let unsure = with_margins(&[5, 6, 7], &[0.2, 2.0, 2.0]);
        let other_start = with_margins(&[9, 6, 7], &[0.1, 2.0, 2.0]);
        assert!(!Ai.agree(unit, &unsure, &other_start));
        // An answer cut short has no token to part at.
        let short = with_margins(&[5, 6], &[3.0, 2.0]);
        assert!(!Ai.agree(unit, &a, &short));
    }

    #[test]
    fn answers_come_back_in_prompt_order() {
        let p = params(&["a", "b"]);
        let result = Ai
            .aggregate(&p, &[output(&[1, 2], "one"), output(&[3], "two")])
            .unwrap();
        let r: JobResult = serde_json::from_value(result).unwrap();
        assert_eq!(r.model, SMOL);
        assert_eq!(
            r.answers[0],
            Answer {
                text: "one".into(),
                tokens: 2
            }
        );
        assert_eq!(
            r.answers[1],
            Answer {
                text: "two".into(),
                tokens: 1
            }
        );
        assert!(Ai.aggregate(&p, &[output(&[1], "one")]).is_err());
    }
}
