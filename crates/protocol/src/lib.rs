//! Messages exchanged between the coordinator, workers and dashboards.
//!
//! These types are the single source of truth; TypeScript bindings for the
//! browser are generated from them with `ts-rs` (run `cargo test -p protocol`).
//! Timestamps are Unix milliseconds. 64-bit integers are exported as `number`
//! because JSON carries them as plain numbers (all values stay below 2^53).

pub mod templates;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;
use uuid::Uuid;

/// Bumped on any incompatible change to the worker protocol.
pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Backend {
    Webgpu,
    Wasm,
    Native,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DeviceInfo {
    pub backend: Backend,
    /// GPU vendor (e.g. "nvidia"), or CPU description for non-GPU backends.
    pub vendor: String,
    pub architecture: String,
}

/// Identifies the exact code a worker must run for a unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TemplateRef {
    pub id: String,
    pub version: u32,
    /// Hex SHA-256 of the kernel source; workers refuse units whose hash differs.
    pub sha256: String,
}

/// Worker → coordinator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum WorkerMessage {
    Hello {
        protocol_version: u32,
        client_version: String,
        device: DeviceInfo,
        /// The device's id across connections, kept by the client; its reputation is
        /// tracked under it (spec 0004). Without one, every connection starts afresh.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        client_id: Option<Uuid>,
    },
    RequestWork,
    Heartbeat {
        lease_ids: Vec<Uuid>,
    },
    Result {
        lease_id: Uuid,
        output: Value,
        elapsed_ms: u32,
    },
    Failed {
        lease_id: Uuid,
        reason: String,
    },
}

/// Coordinator → worker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum CoordinatorMessage {
    Welcome {
        worker_id: Uuid,
        heartbeat_interval_ms: u32,
    },
    Assign {
        lease_id: Uuid,
        unit_id: Uuid,
        template: TemplateRef,
        params: Value,
        #[ts(type = "number")]
        lease_expires_at: i64,
    },
    NoWork {
        retry_after_ms: u32,
    },
    Ack {
        lease_id: Uuid,
        accepted: bool,
    },
    /// Protocol violation; the coordinator closes the connection after sending it.
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum JobStatus {
    Queued,
    Running,
    Done,
    Failed,
}

impl JobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Done => "done",
            Self::Failed => "failed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "queued" => Self::Queued,
            "running" => Self::Running,
            "done" => Self::Done,
            "failed" => Self::Failed,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct JobSummary {
    pub id: Uuid,
    pub template: String,
    pub status: JobStatus,
    pub params: Value,
    pub units_total: u32,
    pub units_done: u32,
    pub result: Option<Value>,
    /// Why the job failed, when it did.
    pub error: Option<String>,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number | null")]
    pub finished_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WorkerSummary {
    pub id: Uuid,
    pub device: DeviceInfo,
    #[ts(type = "number")]
    pub connected_at: i64,
    pub units_done: u32,
}

/// Coordinator → dashboard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum DashboardEvent {
    Snapshot {
        workers: Vec<WorkerSummary>,
        jobs: Vec<JobSummary>,
    },
    WorkerJoined {
        worker: WorkerSummary,
    },
    WorkerLeft {
        worker_id: Uuid,
    },
    UnitCompleted {
        job_id: Uuid,
        unit_id: Uuid,
        worker_id: Uuid,
        units_done: u32,
        units_total: u32,
    },
    JobUpdated {
        job: JobSummary,
    },
}

/// `POST /api/jobs` body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CreateJob {
    pub template: String,
    pub params: Value,
}

/// Error body returned by the REST API.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ApiError {
    pub error: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn worker_message_wire_format() {
        let msg = WorkerMessage::RequestWork;
        assert_eq!(
            serde_json::to_value(&msg).unwrap(),
            json!({"type": "request_work"})
        );

        let parsed: WorkerMessage = serde_json::from_value(json!({
            "type": "hello",
            "protocol_version": 1,
            "client_version": "web-0.1.0",
            "device": {"backend": "webgpu", "vendor": "nvidia", "architecture": "lovelace"}
        }))
        .unwrap();
        assert!(
            matches!(parsed, WorkerMessage::Hello { device, client_id: None, .. } if device.backend == Backend::Webgpu)
        );
    }

    #[test]
    fn unknown_message_is_rejected() {
        assert!(serde_json::from_value::<WorkerMessage>(json!({"type": "rm_rf"})).is_err());
    }

    #[test]
    fn job_status_round_trip() {
        for s in [
            JobStatus::Queued,
            JobStatus::Running,
            JobStatus::Done,
            JobStatus::Failed,
        ] {
            assert_eq!(JobStatus::parse(s.as_str()), Some(s));
        }
    }
}
