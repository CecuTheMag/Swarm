//! Per-template parameter, output and result types.

pub mod pi {
    use serde::{Deserialize, Serialize};
    use ts_rs::TS;

    pub const ID: &str = "pi";
    pub const VERSION: u32 = 1;

    /// Upper bounds keep the WGSL loop counter far from u32 overflow and jobs
    /// within a sane size.
    pub const MAX_UNITS: u32 = 10_000;
    pub const MAX_SAMPLES_PER_UNIT: u32 = 1 << 30;

    /// Parameters supplied when submitting a job.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
    #[serde(deny_unknown_fields)]
    #[ts(export, rename = "PiJobParams")]
    pub struct JobParams {
        pub units: u32,
        pub samples_per_unit: u32,
        pub seed: u32,
    }

    /// Parameters of one work unit, sent to the worker.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
    #[serde(deny_unknown_fields)]
    #[ts(export, rename = "PiUnitParams")]
    pub struct UnitParams {
        pub seed: u32,
        pub unit_index: u32,
        pub samples: u32,
    }

    /// What a worker returns for one unit.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
    #[serde(deny_unknown_fields)]
    #[ts(export, rename = "PiUnitOutput")]
    pub struct UnitOutput {
        pub inside: u32,
    }

    /// Aggregated job result.
    #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
    #[ts(export, rename = "PiResult")]
    pub struct JobResult {
        #[ts(type = "number")]
        pub inside: u64,
        #[ts(type = "number")]
        pub total: u64,
        pub pi: f64,
    }
}

pub mod pt {
    //! Path tracer (spec 0003).

    use base64::{Engine, engine::general_purpose::STANDARD};
    use serde::{Deserialize, Serialize};
    use ts_rs::TS;

    pub const ID: &str = "pt";
    pub const VERSION: u32 = 1;

    pub const MIN_SIZE: u32 = 16;
    pub const MAX_SIZE: u32 = 2048;
    pub const MIN_TILE: u32 = 8;
    pub const MAX_TILE: u32 = 128;
    pub const MAX_SPP: u32 = 1024;
    pub const MAX_PASSES: u32 = 256;
    pub const MAX_UNITS: u32 = 20_000;

    /// Parameters supplied when submitting a job.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
    #[serde(deny_unknown_fields)]
    #[ts(export, rename = "PtJobParams")]
    pub struct JobParams {
        pub width: u32,
        pub height: u32,
        /// Tile edge in pixels; edge tiles may be smaller.
        pub tile: u32,
        /// Samples per pixel added by each pass.
        pub spp: u32,
        pub passes: u32,
        pub seed: u32,
    }

    /// One tile × one pass.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
    #[serde(deny_unknown_fields)]
    #[ts(export, rename = "PtUnitParams")]
    pub struct UnitParams {
        pub width: u32,
        pub height: u32,
        pub x: u32,
        pub y: u32,
        pub w: u32,
        pub h: u32,
        pub spp: u32,
        pub pass: u32,
        pub seed: u32,
    }

    /// What a worker returns for one unit.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
    #[serde(deny_unknown_fields)]
    #[ts(export, rename = "PtUnitOutput")]
    pub struct UnitOutput {
        /// Base64 of little-endian `f32` radiance sums, `w × h × 3`, row-major RGB.
        pub rgb: String,
    }

    /// Aggregated job result; the image itself is served by
    /// `GET /api/jobs/{id}/image.png`.
    #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
    #[ts(export, rename = "PtResult")]
    pub struct JobResult {
        pub width: u32,
        pub height: u32,
        pub samples_per_pixel: u32,
        /// Mean radiance per channel over the image.
        pub mean: [f32; 3],
    }

    pub fn encode_rgb(values: &[f32]) -> String {
        let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        STANDARD.encode(bytes)
    }

    pub fn decode_rgb(text: &str) -> Result<Vec<f32>, String> {
        let bytes = STANDARD
            .decode(text)
            .map_err(|e| format!("rgb is not base64: {e}"))?;
        if bytes.len() % 4 != 0 {
            return Err("rgb length is not a whole number of f32".into());
        }
        Ok(bytes
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn rgb_round_trip() {
            let values = [0.0, 1.5, -2.25, f32::MAX, 1e-30];
            assert_eq!(decode_rgb(&encode_rgb(&values)).unwrap(), values);
            assert!(decode_rgb("not base64!").is_err());
            assert!(decode_rgb(&STANDARD.encode([1u8, 2, 3])).is_err());
        }
    }
}
