use crate::{db::Released, render::Renders};
use protocol::{DashboardEvent, WorkerSummary};
use sqlx::PgPool;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::broadcast;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Config {
    /// How long a worker may hold a unit without a heartbeat.
    pub lease_ttl: Duration,
    /// Heartbeats never extend a lease beyond this age, so a worker that stays
    /// connected but never finishes cannot hold a unit forever (spec 0002).
    pub max_lease_duration: Duration,
    /// A unit whose leases ended this many times without a result fails its job.
    pub max_releases: u32,
    /// How often expired leases are returned to the queue.
    pub reap_interval: Duration,
    /// Results from different clients that must agree before a unit counts (spec 0004).
    /// 1 turns verification off: one result decides the unit.
    pub redundancy: u32,
    /// Extra results a disputed unit may ask for before it fails.
    pub max_extra_results: u32,
    /// Share of work requests answered with an already verified unit.
    pub honeypot_rate: f64,
    /// The same for clients that have not yet earned trust.
    pub new_client_honeypot_rate: f64,
    /// Share of a trusted client's units that are still replicated.
    pub spot_check_rate: f64,
    /// Correct results a client needs before it is trusted.
    pub trust_min_correct: u32,
    /// Trust (Beta posterior mean) a client needs before it is trusted.
    pub trust_threshold: f64,
    /// Incorrect results after which a client with low trust is banned.
    pub ban_min_incorrect: u32,
    /// Trust below which such a client is banned.
    pub ban_trust_below: f64,
    /// Wrong answers to known-answer units after which a client is banned whatever its
    /// trust: colluders can agree with each other, but not with a known answer.
    pub ban_honeypot_failures: u32,
    /// How often workers must send a message (heartbeat or request).
    pub heartbeat_interval: Duration,
    /// Connections silent for this long are considered dead.
    pub idle_timeout: Duration,
    /// Back-off suggested to workers when the queue is empty.
    pub no_work_retry: Duration,
    /// Maximum units a single worker may hold at once.
    pub max_leases_per_worker: usize,
}

impl Config {
    /// Defaults, with `SWARM_REDUNDANCY` from the environment if set (1 for a single
    /// device in development; production keeps 2).
    pub fn from_env() -> Self {
        let mut cfg = Self::default();
        if let Some(r) = std::env::var("SWARM_REDUNDANCY")
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|r| *r >= 1)
        {
            cfg.redundancy = r;
        }
        cfg
    }

    /// Mean of the Beta posterior over a client's correct and incorrect results.
    pub fn trust(correct: i32, incorrect: i32) -> f64 {
        (f64::from(correct) + 1.0) / (f64::from(correct) + f64::from(incorrect) + 2.0)
    }

    pub fn is_trusted(&self, correct: i32, incorrect: i32) -> bool {
        correct >= self.trust_min_correct as i32
            && Self::trust(correct, incorrect) >= self.trust_threshold
    }

    pub fn should_ban(&self, correct: i32, incorrect: i32, honeypot_failures: i32) -> bool {
        honeypot_failures >= self.ban_honeypot_failures as i32
            || (incorrect >= self.ban_min_incorrect as i32
                && Self::trust(correct, incorrect) < self.ban_trust_below)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            lease_ttl: Duration::from_secs(60),
            max_lease_duration: Duration::from_secs(600),
            max_releases: 8,
            reap_interval: Duration::from_secs(5),
            redundancy: 2,
            max_extra_results: 3,
            honeypot_rate: 0.05,
            new_client_honeypot_rate: 0.2,
            spot_check_rate: 0.1,
            trust_min_correct: 20,
            trust_threshold: 0.95,
            ban_min_incorrect: 3,
            ban_trust_below: 0.5,
            ban_honeypot_failures: 2,
            heartbeat_interval: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(30),
            no_work_retry: Duration::from_secs(2),
            max_leases_per_worker: 1,
        }
    }
}

pub type AppState = Arc<State>;

pub struct State {
    pub db: PgPool,
    pub config: Config,
    /// Fan-out of live events to dashboard connections.
    pub events: broadcast::Sender<DashboardEvent>,
    /// Workers connected to this instance. Moves to Redis with multiple instances.
    pub workers: Mutex<HashMap<Uuid, WorkerSummary>>,
    /// Live images of path-tracer jobs.
    pub renders: Renders,
}

impl State {
    pub fn new(db: PgPool, config: Config) -> AppState {
        let (events, _) = broadcast::channel(1024);
        Arc::new(Self {
            db,
            config,
            events,
            workers: Mutex::new(HashMap::new()),
            renders: Renders::default(),
        })
    }

    /// Sends an event to all dashboards; having no dashboards connected is fine.
    pub fn publish(&self, event: DashboardEvent) {
        let _ = self.events.send(event);
    }

    /// Announces jobs that failed because a released unit was released too often.
    pub fn publish_released(&self, released: Released) {
        for job in released.failed_jobs {
            tracing::warn!(job_id = %job.id, error = job.error.as_deref().unwrap_or(""), "job failed");
            self.publish(DashboardEvent::JobUpdated { job });
        }
    }

    pub fn worker_snapshot(&self) -> Vec<WorkerSummary> {
        let mut workers: Vec<_> = self.workers.lock().unwrap().values().cloned().collect();
        workers.sort_by_key(|w| w.connected_at);
        workers
    }
}
