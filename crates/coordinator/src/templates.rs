//! Registry of compute templates.
//!
//! A template defines how a job is split into units, what a valid unit output
//! looks like, and how outputs are reduced into the job result. Workers only
//! ever run kernels shipped with the site; jobs carry parameters, never code.

mod pi;

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::LazyLock;

pub trait Template: Send + Sync {
    fn id(&self) -> &'static str;
    fn version(&self) -> u32;
    /// Kernel source the worker runs; its hash is pinned in every assignment.
    fn kernel_source(&self) -> &'static str;
    /// Validates job parameters and returns per-unit parameters in order.
    fn split(&self, params: &Value) -> Result<Vec<Value>, String>;
    /// Checks that `output` is well-formed and plausible for `unit_params`.
    fn validate_output(&self, unit_params: &Value, output: &Value) -> Result<(), String>;
    /// Reduces unit outputs (in unit order) into the job result.
    fn aggregate(&self, job_params: &Value, outputs: &[Value]) -> Result<Value, String>;
}

pub struct Registered {
    pub template: &'static dyn Template,
    pub sha256: String,
}

static REGISTRY: LazyLock<Vec<Registered>> = LazyLock::new(|| {
    let all: [&'static dyn Template; 1] = [&pi::Pi];
    all.into_iter()
        .map(|template| Registered {
            template,
            sha256: hex::encode(Sha256::digest(template.kernel_source().as_bytes())),
        })
        .collect()
});

pub fn get(id: &str) -> Option<&'static Registered> {
    REGISTRY.iter().find(|r| r.template.id() == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_hashes_are_hex_sha256() {
        let r = get("pi").unwrap();
        assert_eq!(r.sha256.len(), 64);
        assert!(r.sha256.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn unknown_template() {
        assert!(get("miner").is_none());
    }
}
