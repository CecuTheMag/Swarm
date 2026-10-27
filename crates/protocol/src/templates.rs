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
