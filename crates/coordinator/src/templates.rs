//! Registry of compute templates.
//!
//! A template defines how a job is split into units, what a valid unit output
//! looks like, and how outputs are reduced into the job result. Workers run kernels
//! shipped with the site, or a network's checked program (spec 0012) pinned by its
//! hash; jobs of the other templates carry parameters, never code.

pub mod nn;
mod pi;
pub mod program;
pub mod pt;

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::LazyLock;

pub trait Template: Send + Sync {
    fn id(&self) -> &'static str;
    fn version(&self) -> u32;
    /// Kernel source the worker runs; its hash is pinned in every assignment. None for
    /// programs, whose module comes with the job.
    fn kernel_source(&self) -> Option<&'static str>;
    /// Validates job parameters and returns per-unit parameters in order.
    fn split(&self, params: &Value) -> Result<Vec<Value>, String>;
    /// Checks that `output` is well-formed and plausible for `unit_params`.
    fn validate_output(&self, unit_params: &Value, output: &Value) -> Result<(), String>;
    /// Whether two valid outputs for the same unit count as the same answer (spec 0004).
    fn agree(&self, unit_params: &Value, a: &Value, b: &Value) -> bool;
    /// Reduces unit outputs (in unit order) into the job result.
    fn aggregate(&self, job_params: &Value, outputs: &[Value]) -> Result<Value, String>;
}

pub struct Registered {
    pub template: &'static dyn Template,
    /// Hash of the kernel source, for templates shipped with the site.
    pub sha256: Option<String>,
}

static REGISTRY: LazyLock<Vec<Registered>> = LazyLock::new(|| {
    let all: [&'static dyn Template; 4] = [&pi::Pi, &pt::Pt, &nn::Nn, &program::Program];
    all.into_iter()
        .map(|template| Registered {
            template,
            sha256: template
                .kernel_source()
                .map(|source| hex::encode(Sha256::digest(source.as_bytes()))),
        })
        .collect()
});

pub fn get(id: &str) -> Option<&'static Registered> {
    REGISTRY.iter().find(|r| r.template.id() == id)
}

/// Hashes of the kernels shipped with the site today.
pub fn current_hashes() -> Vec<String> {
    REGISTRY.iter().filter_map(|r| r.sha256.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_hashes_are_hex_sha256() {
        for id in ["pi", "pt", "nn"] {
            let sha256 = get(id).unwrap().sha256.clone().unwrap();
            assert_eq!(sha256.len(), 64);
            assert!(sha256.chars().all(|c| c.is_ascii_hexdigit()));
        }
        assert_eq!(get("program").unwrap().sha256, None);
        assert_eq!(current_hashes().len(), 3);
    }

    #[test]
    fn unknown_template() {
        assert!(get("miner").is_none());
    }
}
