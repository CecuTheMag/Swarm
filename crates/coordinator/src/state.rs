use crate::{
    db::{Ban, Released},
    render::Renders,
};
use protocol::{
    DashboardEvent, JobProgress, JobProgressSummary, NetworkSummary, WorkerProgress, WorkerSummary,
};
use sqlx::PgPool;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex, atomic::AtomicUsize},
    time::{Duration, Instant},
};
use tokio::sync::{broadcast, mpsc};
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
    /// Fan-out of live events to dashboards, tagged with their network.
    pub events: broadcast::Sender<(Uuid, DashboardEvent)>,
    /// Workers connected to this instance. Moves to Redis with multiple instances.
    pub workers: Mutex<HashMap<Uuid, WorkerSummary>>,
    /// Who each connected worker is: its client, network and device.
    links: Mutex<HashMap<Uuid, WorkerLink>>,
    /// Connected workers whose client was banned; each session ends at its next message.
    banned: Mutex<HashSet<Uuid>>,
    /// Live images of path-tracer jobs.
    pub renders: Renders,
    /// Units decided since dashboards were last told, by network (see
    /// [`State::flush_progress`]).
    progress: Mutex<HashMap<Uuid, Progress>>,
    /// Units decided per device since they were last written to the database.
    device_units: Mutex<HashMap<Uuid, i32>>,
    /// Decided units per network over the last [`RATE_WINDOW`], for join summaries.
    rates: Mutex<HashMap<Uuid, VecDeque<(Instant, u32)>>>,
    /// What a network's devices see of it, computed at most once a second.
    summaries: Mutex<HashMap<Uuid, (Instant, NetworkSummary)>>,
    /// How the chaos panel and the console reach each connected worker's session.
    commands: Mutex<HashMap<Uuid, mpsc::UnboundedSender<SessionCommand>>>,
    /// Cheaters added by the chaos panel that are still running.
    pub chaos_cheaters: AtomicUsize,
    /// Failed sign-ins per email (spec 0010).
    pub logins: crate::auth::LoginLimiter,
}

#[derive(Debug, Clone, Copy)]
struct WorkerLink {
    client_id: Uuid,
    network_id: Uuid,
    device_id: Uuid,
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

/// Something a worker's session must act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionCommand {
    Disrupt(Disruption),
    /// Its network's rules or its device changed: reload them and tell the device.
    Refresh,
}

/// How often dashboards hear about decided units.
pub const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);
/// The window over which join summaries measure speed.
pub const RATE_WINDOW: Duration = Duration::from_secs(10);

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
            links: Mutex::new(HashMap::new()),
            banned: Mutex::new(HashSet::new()),
            renders: Renders::default(),
            progress: Mutex::new(HashMap::new()),
            device_units: Mutex::new(HashMap::new()),
            rates: Mutex::new(HashMap::new()),
            summaries: Mutex::new(HashMap::new()),
            commands: Mutex::new(HashMap::new()),
            chaos_cheaters: AtomicUsize::new(0),
            logins: crate::auth::LoginLimiter::default(),
        })
    }

    /// Where the session of `worker_id` hears of commands; until it leaves.
    pub fn commands_of(&self, worker_id: Uuid) -> mpsc::UnboundedReceiver<SessionCommand> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.commands.lock().unwrap().insert(worker_id, tx);
        rx
    }

    /// Disrupts a connected worker of `network_id`; false if no such worker is
    /// connected here.
    pub fn disrupt(&self, network_id: Uuid, worker_id: Uuid, disruption: Disruption) -> bool {
        if self.network_of(worker_id) != Some(network_id) {
            return false;
        }
        let tx = self.commands.lock().unwrap().get(&worker_id).cloned();
        tx.is_some_and(|tx| tx.send(SessionCommand::Disrupt(disruption)).is_ok())
    }

    /// Tells every connected worker of a network that its rules changed.
    pub fn refresh_network(&self, network_id: Uuid) {
        self.refresh_where(|link| link.network_id == network_id);
    }

    /// Tells a device's connected workers that it changed.
    pub fn refresh_device(&self, device_id: Uuid) {
        self.refresh_where(|link| link.device_id == device_id);
    }

    fn refresh_where(&self, matches: impl Fn(&WorkerLink) -> bool) {
        let workers: Vec<Uuid> = self
            .links
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, link)| matches(link))
            .map(|(id, _)| *id)
            .collect();
        let commands = self.commands.lock().unwrap();
        for id in workers {
            if let Some(tx) = commands.get(&id) {
                let _ = tx.send(SessionCommand::Refresh);
            }
        }
    }

    pub fn network_of(&self, worker_id: Uuid) -> Option<Uuid> {
        self.links
            .lock()
            .unwrap()
            .get(&worker_id)
            .map(|l| l.network_id)
    }

    /// Connected workers of a device; the console shows them as online.
    pub fn online_devices(&self, network_id: Uuid) -> HashSet<Uuid> {
        self.links
            .lock()
            .unwrap()
            .values()
            .filter(|l| l.network_id == network_id)
            .map(|l| l.device_id)
            .collect()
    }

    /// The connected workers of a device.
    pub fn workers_of_device(&self, device_id: Uuid) -> Vec<Uuid> {
        self.links
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, l)| l.device_id == device_id)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Sends an event to a network's dashboards; having none connected is fine. Units
    /// decided before it are announced first, so a dashboard sees events in order.
    /// A job a network runs on the public network is also announced to that network
    /// (spec 0012).
    pub fn publish(&self, network_id: Uuid, event: DashboardEvent) {
        self.flush_progress();
        if let DashboardEvent::JobUpdated { job } = &event
            && let Some(owner) = job.owner_network_id
            && owner != network_id
        {
            let _ = self.events.send((owner, event.clone()));
        }
        let _ = self.events.send((network_id, event));
    }

    /// Registers a connected worker and announces it to its network.
    pub fn worker_joined(
        &self,
        worker: WorkerSummary,
        client_id: Uuid,
        network_id: Uuid,
        device_id: Uuid,
    ) {
        self.links.lock().unwrap().insert(
            worker.id,
            WorkerLink {
                client_id,
                network_id,
                device_id,
            },
        );
        self.workers
            .lock()
            .unwrap()
            .insert(worker.id, worker.clone());
        self.summaries.lock().unwrap().remove(&network_id);
        self.publish(network_id, DashboardEvent::WorkerJoined { worker });
    }

    /// Forgets a worker whose connection ended and announces it.
    pub fn worker_left(&self, worker_id: Uuid, units_returned: u32) {
        self.workers.lock().unwrap().remove(&worker_id);
        let link = self.links.lock().unwrap().remove(&worker_id);
        self.banned.lock().unwrap().remove(&worker_id);
        self.commands.lock().unwrap().remove(&worker_id);
        if let Some(link) = link {
            self.summaries.lock().unwrap().remove(&link.network_id);
            self.publish(
                link.network_id,
                DashboardEvent::WorkerLeft {
                    worker_id,
                    units_returned,
                },
            );
        }
    }

    /// Marks the connected workers of banned clients, so their sessions end, and
    /// announces each: on the projector, this is the cheater being caught.
    pub fn clients_banned(&self, bans: &[Ban]) {
        for ban in bans {
            let workers: Vec<(Uuid, Uuid)> = self
                .links
                .lock()
                .unwrap()
                .iter()
                .filter(|(_, link)| link.client_id == ban.client_id)
                .map(|(worker, link)| (*worker, link.network_id))
                .collect();
            for (worker_id, network_id) in workers {
                self.banned.lock().unwrap().insert(worker_id);
                self.publish(
                    network_id,
                    DashboardEvent::WorkerBanned {
                        worker_id,
                        wrong_results: ban.wrong_results,
                        units_redone: ban.units_redone,
                    },
                );
            }
        }
    }

    /// Whether the worker's client was banned while it was connected.
    pub fn is_banned(&self, worker_id: Uuid) -> bool {
        self.banned.lock().unwrap().contains(&worker_id)
    }

    /// Records a decided unit of a job in `network_id` for the next
    /// [`DashboardEvent::UnitsCompleted`] of that network.
    pub fn unit_decided(
        &self,
        network_id: Uuid,
        job_id: Uuid,
        worker_id: Uuid,
        units_done: u32,
        units_total: u32,
    ) {
        {
            let mut all = self.progress.lock().unwrap();
            let p = all.entry(network_id).or_default();
            p.units += 1;
            let job = p.jobs.entry(job_id).or_insert((0, units_total));
            job.0 = job.0.max(units_done);
            *p.workers.entry(worker_id).or_default() += 1;
        }
        let device = self
            .links
            .lock()
            .unwrap()
            .get(&worker_id)
            .map(|l| l.device_id);
        if let Some(device) = device {
            *self.device_units.lock().unwrap().entry(device).or_default() += 1;
        }
    }

    /// Announces the units decided since the last call, if any, to each network's
    /// dashboards. Runs every [`PROGRESS_INTERVAL`]: one event per batch instead of one
    /// per unit, which with a thousand devices would be more than a browser can draw.
    pub fn flush_progress(&self) {
        let all = std::mem::take(&mut *self.progress.lock().unwrap());
        if all.is_empty() {
            return;
        }
        let now = Instant::now();
        let mut rates = self.rates.lock().unwrap();
        for (network_id, p) in all {
            if p.units == 0 {
                continue;
            }
            rates
                .entry(network_id)
                .or_default()
                .push_back((now, p.units));
            let _ = self.events.send((
                network_id,
                DashboardEvent::UnitsCompleted {
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
                },
            ));
        }
    }

    /// Takes the per-device unit counts gathered since the last call.
    pub fn take_device_units(&self) -> Vec<(Uuid, i32)> {
        std::mem::take(&mut *self.device_units.lock().unwrap())
            .into_iter()
            .collect()
    }

    /// Announces jobs that failed because a released unit was released too often.
    pub fn publish_released(&self, released: Released) {
        for job in released.failed_jobs {
            tracing::warn!(job_id = %job.id, error = job.error.as_deref().unwrap_or(""), "job failed");
            self.publish(job.network_id, DashboardEvent::JobUpdated { job });
        }
    }

    /// A network's connected workers, oldest first.
    pub fn worker_snapshot(&self, network_id: Uuid) -> Vec<WorkerSummary> {
        let links = self.links.lock().unwrap();
        let mut workers: Vec<_> = self
            .workers
            .lock()
            .unwrap()
            .values()
            .filter(|w| links.get(&w.id).is_some_and(|l| l.network_id == network_id))
            .cloned()
            .collect();
        workers.sort_by_key(|w| w.connected_at);
        workers
    }

    /// Decided units per second in a network over the last [`RATE_WINDOW`].
    fn units_per_second(&self, network_id: Uuid) -> f64 {
        let mut rates = self.rates.lock().unwrap();
        let Some(window) = rates.get_mut(&network_id) else {
            return 0.0;
        };
        while window
            .front()
            .is_some_and(|(at, _)| at.elapsed() > RATE_WINDOW)
        {
            window.pop_front();
        }
        let units: u32 = window.iter().map(|(_, n)| n).sum();
        f64::from(units) / RATE_WINDOW.as_secs_f64()
    }

    /// What a network's devices see of it: computed at most once a second however
    /// many devices ask.
    pub async fn network_summary(
        &self,
        network_id: Uuid,
        name: &str,
    ) -> Result<NetworkSummary, sqlx::Error> {
        if let Some((at, summary)) = self.summaries.lock().unwrap().get(&network_id)
            && at.elapsed() < Duration::from_secs(1)
        {
            return Ok(summary.clone());
        }
        let job: Option<(String, i32, i32)> = sqlx::query_as(
            "SELECT template, units_done, units_total FROM jobs
             WHERE network_id = $1 AND status IN ('queued', 'running')
             ORDER BY created_at LIMIT 1",
        )
        .bind(network_id)
        .fetch_optional(&self.db)
        .await?;
        let (mut gpu, mut cpu, mut server) = (0, 0, 0);
        for w in self.worker_snapshot(network_id) {
            match w.device.backend {
                protocol::Backend::Webgpu => gpu += 1,
                protocol::Backend::Wasm => cpu += 1,
                protocol::Backend::Native => server += 1,
            }
        }
        let summary = NetworkSummary {
            name: name.to_owned(),
            devices_gpu: gpu,
            devices_cpu: cpu,
            devices_server: server,
            units_per_second: self.units_per_second(network_id),
            job: job.map(|(template, units_done, units_total)| JobProgressSummary {
                template,
                units_done: units_done as u32,
                units_total: units_total as u32,
            }),
        };
        self.summaries
            .lock()
            .unwrap()
            .insert(network_id, (Instant::now(), summary.clone()));
        Ok(summary)
    }
}
