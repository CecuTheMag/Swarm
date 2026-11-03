use crate::{
    db::{Ban, Released},
    render::Renders,
};
use protocol::{DashboardEvent, JobProgress, WorkerProgress, WorkerSummary};
use sqlx::PgPool;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, atomic::AtomicUsize},
    time::Duration,
};
use tokio::sync::{broadcast, oneshot};
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
    /// Maximum units a single worker may hold at once. Workers fetch units in advance so
    /// round trips overlap the work; a fast device behind a slow link needs several in
    /// flight to stay busy.
    pub max_leases_per_worker: usize,
    /// Password for submitting jobs (`SWARM_SUBMIT_TOKEN`). Anyone may join as a worker,
    /// but on a public address only the organiser starts jobs. None leaves submitting open.
    /// The chaos panel needs it too (spec 0008).
    pub submit_token: Option<String>,
    /// This coordinator's own worker endpoint, which cheaters added by the chaos panel
    /// connect to like any device. None turns adding cheaters off.
    pub self_url: Option<String>,
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
        cfg.submit_token = std::env::var("SWARM_SUBMIT_TOKEN")
            .ok()
            .filter(|t| !t.is_empty());
        cfg
    }

    /// Mean of the Beta posterior over a client's correct and incorrect results.
    pub fn trust(correct: i32, incorrect: i32) -> f64 {
        (f64::from(correct) + 1.0) / (f64::from(correct) + f64::from(incorrect) + 2.0)
    }

    // Counts are compared as i64: a `u32` limit above `i32::MAX` must not wrap negative
    // and trust (or ban) everyone.
    pub fn is_trusted(&self, correct: i32, incorrect: i32) -> bool {
        i64::from(correct) >= i64::from(self.trust_min_correct)
            && Self::trust(correct, incorrect) >= self.trust_threshold
    }

    pub fn should_ban(&self, correct: i32, incorrect: i32, honeypot_failures: i32) -> bool {
        i64::from(honeypot_failures) >= i64::from(self.ban_honeypot_failures)
            || (i64::from(incorrect) >= i64::from(self.ban_min_incorrect)
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
            max_leases_per_worker: 8,
            submit_token: None,
            self_url: None,
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
    /// The client of each connected worker.
    clients: Mutex<HashMap<Uuid, Uuid>>,
    /// Connected workers whose client was banned; each session ends at its next message.
    banned: Mutex<HashSet<Uuid>>,
    /// Live images of path-tracer jobs.
    pub renders: Renders,
    /// Units decided since dashboards were last told (see [`State::flush_progress`]).
    progress: Mutex<Progress>,
    /// How the chaos panel reaches each connected worker's session.
    disruptions: Mutex<HashMap<Uuid, oneshot::Sender<Disruption>>>,
    /// Cheaters added by the chaos panel that are still running.
    pub chaos_cheaters: AtomicUsize,
}

/// What the chaos panel does to a connected worker (spec 0008).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disruption {
    /// The device is switched off: the connection closes and its units go back at once.
    Kill,
    /// The device loses its signal: nothing more arrives from it or reaches it, and the
    /// coordinator finds out only when the connection has been silent too long.
    CutOff,
}

/// How often dashboards hear about decided units.
pub const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Default)]
struct Progress {
    units: u32,
    /// Highest `units_done` seen and `units_total`, by job.
    jobs: HashMap<Uuid, (u32, u32)>,
    workers: HashMap<Uuid, u32>,
}

impl State {
    pub fn new(db: PgPool, config: Config) -> AppState {
        let (events, _) = broadcast::channel(1024);
        Arc::new(Self {
            db,
            config,
            events,
            workers: Mutex::new(HashMap::new()),
            clients: Mutex::new(HashMap::new()),
            banned: Mutex::new(HashSet::new()),
            renders: Renders::default(),
            progress: Mutex::new(Progress::default()),
            disruptions: Mutex::new(HashMap::new()),
            chaos_cheaters: AtomicUsize::new(0),
        })
    }

    /// Where the session of `worker_id` hears of a disruption; until it leaves.
    pub fn disruptions_of(&self, worker_id: Uuid) -> oneshot::Receiver<Disruption> {
        let (tx, rx) = oneshot::channel();
        self.disruptions.lock().unwrap().insert(worker_id, tx);
        rx
    }

    /// Disrupts a connected worker; false if it is not connected here (or already disrupted).
    pub fn disrupt(&self, worker_id: Uuid, disruption: Disruption) -> bool {
        let tx = self.disruptions.lock().unwrap().remove(&worker_id);
        tx.is_some_and(|tx| tx.send(disruption).is_ok())
    }

    /// Sends an event to all dashboards; having no dashboards connected is fine. Units
    /// decided before it are announced first, so a dashboard sees events in order.
    pub fn publish(&self, event: DashboardEvent) {
        self.flush_progress();
        let _ = self.events.send(event);
    }

    /// Registers a connected worker and announces it.
    pub fn worker_joined(&self, worker: WorkerSummary, client_id: Uuid) {
        self.clients.lock().unwrap().insert(worker.id, client_id);
        self.workers
            .lock()
            .unwrap()
            .insert(worker.id, worker.clone());
        self.publish(DashboardEvent::WorkerJoined { worker });
    }

    /// Forgets a worker whose connection ended and announces it.
    pub fn worker_left(&self, worker_id: Uuid, units_returned: u32) {
        self.workers.lock().unwrap().remove(&worker_id);
        self.clients.lock().unwrap().remove(&worker_id);
        self.banned.lock().unwrap().remove(&worker_id);
        self.disruptions.lock().unwrap().remove(&worker_id);
        self.publish(DashboardEvent::WorkerLeft {
            worker_id,
            units_returned,
        });
    }

    /// Marks the connected workers of banned clients, so their sessions end, and
    /// announces each: on the projector, this is the cheater being caught.
    pub fn clients_banned(&self, bans: &[Ban]) {
        for ban in bans {
            let workers: Vec<Uuid> = self
                .clients
                .lock()
                .unwrap()
                .iter()
                .filter(|(_, client)| **client == ban.client_id)
                .map(|(worker, _)| *worker)
                .collect();
            for worker_id in workers {
                self.banned.lock().unwrap().insert(worker_id);
                self.publish(DashboardEvent::WorkerBanned {
                    worker_id,
                    wrong_results: ban.wrong_results,
                    units_redone: ban.units_redone,
                });
            }
        }
    }

    /// Whether the worker's client was banned while it was connected.
    pub fn is_banned(&self, worker_id: Uuid) -> bool {
        self.banned.lock().unwrap().contains(&worker_id)
    }

    /// Records a decided unit for the next [`DashboardEvent::UnitsCompleted`].
    pub fn unit_decided(&self, job_id: Uuid, worker_id: Uuid, units_done: u32, units_total: u32) {
        let mut p = self.progress.lock().unwrap();
        p.units += 1;
        let job = p.jobs.entry(job_id).or_insert((0, units_total));
        job.0 = job.0.max(units_done);
        *p.workers.entry(worker_id).or_default() += 1;
    }

    /// Announces the units decided since the last call, if any. Runs every
    /// [`PROGRESS_INTERVAL`]: one event per batch instead of one per unit, which with a
    /// thousand devices would be more than a browser can draw.
    pub fn flush_progress(&self) {
        let p = std::mem::take(&mut *self.progress.lock().unwrap());
        if p.units == 0 {
            return;
        }
        let _ = self.events.send(DashboardEvent::UnitsCompleted {
            units: p.units,
            jobs: p
                .jobs
                .into_iter()
                .map(|(job_id, (units_done, units_total))| JobProgress {
                    job_id,
                    units_done,
                    units_total,
                })
                .collect(),
            workers: p
                .workers
                .into_iter()
                .map(|(worker_id, units)| WorkerProgress { worker_id, units })
                .collect(),
        });
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

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn limits_above_i32_max_switch_trust_and_bans_off() {
        let cfg = Config {
            trust_min_correct: u32::MAX,
            ban_min_incorrect: u32::MAX,
            ban_honeypot_failures: u32::MAX,
            ..Config::default()
        };
        assert!(!cfg.is_trusted(1_000, 0));
        assert!(!cfg.should_ban(0, 1_000, 1_000));
        let default = Config::default();
        assert!(default.is_trusted(default.trust_min_correct as i32, 0));
        assert!(default.should_ban(0, 0, default.ban_honeypot_failures as i32));
    }
}
