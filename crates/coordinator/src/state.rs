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
    /// How often workers must send a message (heartbeat or request).
    pub heartbeat_interval: Duration,
    /// Connections silent for this long are considered dead.
    pub idle_timeout: Duration,
    /// Back-off suggested to workers when the queue is empty.
    pub no_work_retry: Duration,
    /// Maximum units a single worker may hold at once.
    pub max_leases_per_worker: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            lease_ttl: Duration::from_secs(60),
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
}

impl State {
    pub fn new(db: PgPool, config: Config) -> AppState {
        let (events, _) = broadcast::channel(1024);
        Arc::new(Self {
            db,
            config,
            events,
            workers: Mutex::new(HashMap::new()),
        })
    }

    /// Sends an event to all dashboards; having no dashboards connected is fine.
    pub fn publish(&self, event: DashboardEvent) {
        let _ = self.events.send(event);
    }

    pub fn worker_snapshot(&self) -> Vec<WorkerSummary> {
        let mut workers: Vec<_> = self.workers.lock().unwrap().values().cloned().collect();
        workers.sort_by_key(|w| w.connected_at);
        workers
    }
}
