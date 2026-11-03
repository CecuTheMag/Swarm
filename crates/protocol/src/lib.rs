//! Messages exchanged between the coordinator, workers and dashboards.
//!
//! These types are the single source of truth; TypeScript bindings for the
//! browser are generated from them with `ts-rs` (run `cargo test -p protocol`).
//! Timestamps are Unix milliseconds. 64-bit integers are exported as `number`
//! because JSON carries them as plain numbers (all values stay below 2^53).

pub mod api;
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
        /// The network joined, from its join link (spec 0010); none joins the public one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        join_code: Option<String>,
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
    /// What the network's owner allows this device right now (spec 0010). Sent after
    /// `welcome` and whenever it changes; while not `running`, work requests get
    /// `no_work`. The device never computes above `max_power` percent.
    Control {
        control: DeviceControl,
        max_power: u32,
    },
    /// Protocol violation; the coordinator closes the connection after sending it.
    Error {
        message: String,
    },
}

/// Whether a device may compute, as its network's owner decided (spec 0010).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "state", rename_all = "snake_case")]
#[ts(export)]
pub enum DeviceControl {
    Running,
    /// The owner paused this device.
    PausedByOwner,
    /// The owner paused the whole network.
    NetworkPaused,
    /// Outside the network's working hours; `resumes_at` is when the next window opens.
    OutsideHours {
        #[ts(type = "number | null")]
        resumes_at: Option<i64>,
    },
    /// The network admits devices only after approval, which has not happened yet.
    PendingApproval,
    /// The owner removed this device from the network; the connection closes next.
    Removed,
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
    pub network_id: Uuid,
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

/// A job's progress as of a [`DashboardEvent::UnitsCompleted`] batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct JobProgress {
    pub job_id: Uuid,
    pub units_done: u32,
    pub units_total: u32,
}

/// Units a worker decided within a [`DashboardEvent::UnitsCompleted`] batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WorkerProgress {
    pub worker_id: Uuid,
    pub units: u32,
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
        /// Units it held that went back to the queue for other devices.
        units_returned: u32,
    },
    /// The worker's client returned wrong results and is blocked (spec 0004). Its
    /// connection closes next, with a `worker_left`.
    WorkerBanned {
        worker_id: Uuid,
        /// Wrong results it returned, known answers included.
        wrong_results: u32,
        /// Units it had helped decide that are computed again.
        units_redone: u32,
    },
    /// Units decided since the previous event, sent a few times a second rather than
    /// once per unit, so dashboards keep up however many devices take part.
    UnitsCompleted {
        /// Units decided in this batch.
        units: u32,
        jobs: Vec<JobProgress>,
        workers: Vec<WorkerProgress>,
    },
    JobUpdated {
        job: JobSummary,
    },
}

/// What a device that joined a network sees of it (spec 0010): enough to show the
/// network at work, nothing about other devices or the jobs' data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NetworkSummary {
    pub name: String,
    pub devices_gpu: u32,
    pub devices_cpu: u32,
    pub devices_server: u32,
    /// Parts (units) decided per second over the last ten seconds.
    pub units_per_second: f64,
    pub job: Option<JobProgressSummary>,
}

/// The job a network is working on, as a device sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct JobProgressSummary {
    pub template: String,
    pub units_done: u32,
    pub units_total: u32,
}

/// `POST /api/jobs` body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CreateJob {
    pub template: String,
    pub params: Value,
}

/// `POST /api/chaos/cheaters` body: simulated devices that make up every result join
/// the grid, for the chaos panel (spec 0008).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AddCheaters {
    /// How many, 1–3.
    pub count: u32,
    /// All return the same wrong answer, so they agree with each other.
    pub collude: bool,
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
