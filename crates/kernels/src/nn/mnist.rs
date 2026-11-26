//! `mnist`: real handwritten digits, 14 × 14 (spec 0011).
//!
//! MNIST (Y. LeCun, C. Cortes, C. J. C. Burges; CC BY-SA 3.0) packed by
//! `scripts/pack_mnist.py`: each 28 × 28 image averaged over 2 × 2 blocks, rounded; the
//! first 10 000 images of MNIST's training set for training and the first 2 000 of its
//! test set, written by different people, for validation. The file is pinned by
//! [`SHA256`]: every device trains on exactly these bytes.

use super::Dataset;

pub const SIDE: usize = 14;
pub const DIM: usize = SIDE * SIDE;
pub const TRAIN: usize = 10_000;
pub const VALIDATION: usize = 2_000;
/// Size of the packed file: the training pixels, the validation pixels, the training
/// labels, the validation labels.
pub const BYTES: usize = (TRAIN + VALIDATION) * (DIM + 1);
/// SHA-256 of `templates/nn/mnist14.bin`.
pub const SHA256: &str = "b66ab11fb7cb11263a568b394201c2d0e9bb88a85508abbdedef04d68713c717";

/// The dataset from the packed file's bytes.
pub fn parse(bytes: &[u8]) -> Result<Dataset, String> {
    if bytes.len() != BYTES {
        return Err(format!("mnist: {} bytes, expected {BYTES}", bytes.len()));
    }
    let (pixels, labels) = bytes.split_at((TRAIN + VALIDATION) * DIM);
    if labels.iter().any(|&y| usize::from(y) >= super::CLASSES) {
        return Err("mnist: label out of range".into());
    }
    let (train_x, val_x) = pixels.split_at(TRAIN * DIM);
    let (train_y, val_y) = labels.split_at(TRAIN);
    Ok(Dataset {
        dim: DIM,
        train_x: train_x.to_vec(),
        train_y: train_y.to_vec(),
        val_x: val_x.to_vec(),
        val_y: val_y.to_vec(),
    })
}

/// The packed file, built into native programs (the coordinator, simulated devices);
/// browsers download it once and hand it to [`parse`].
pub fn dataset() -> Dataset {
    parse(include_bytes!("../../../../templates/nn/mnist14.bin")).expect("the packed file")
}
