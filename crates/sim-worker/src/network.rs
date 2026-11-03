//! A unit of the neural network sweep (spec 0011) as a device computes it, with the
//! reference kernels. The coordinator uses the same code to retrain the best model.

use kernels::nn::{self, Config, Dataset, Trained, glyphs};
use protocol::templates::nn::{UnitOutput, UnitParams};
use sha2::{Digest, Sha256};
use std::sync::LazyLock;

/// The `glyphs` dataset, generated once per process.
static GLYPHS: LazyLock<Dataset> = LazyLock::new(glyphs::dataset);

/// The dataset a unit trains on.
pub fn dataset(name: &str) -> Result<&'static Dataset, String> {
    match name {
        "glyphs" => Ok(&GLYPHS),
        other => Err(format!("unknown dataset {other}")),
    }
}

/// SHA-256 (hex) of a trained network's final weights.
pub fn digest(trained: &Trained) -> String {
    hex::encode(Sha256::digest(trained.weight_bytes()))
}

/// The training configuration of a unit.
pub fn config(u: &UnitParams) -> Config {
    Config {
        hidden: u.hidden,
        layers: u.layers,
        lr_q16: u.lr_q16,
        batch_log2: u.batch_log2,
        steps: u.steps,
        seed: u.seed,
    }
}

/// Trains a unit as a device would: its output, and the network itself.
pub fn train_unit(u: &UnitParams) -> Result<(UnitOutput, Trained), String> {
    let cfg = config(u);
    cfg.validate()?;
    let trained = nn::train(&cfg, dataset(&u.dataset)?);
    let output = UnitOutput {
        correct: trained.correct,
        curve: trained.curve.to_vec(),
        digest: digest(&trained),
    };
    Ok((output, trained))
}
