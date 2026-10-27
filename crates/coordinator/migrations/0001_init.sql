CREATE TABLE jobs (
    id               uuid PRIMARY KEY,
    template         text        NOT NULL,
    template_version integer     NOT NULL,
    params           jsonb       NOT NULL,
    status           text        NOT NULL CHECK (status IN ('queued', 'running', 'done', 'failed')),
    units_total      integer     NOT NULL CHECK (units_total > 0),
    -- Incremented under the job row lock when a unit completes, so exactly one
    -- completion observes units_done = units_total and aggregates the job.
    units_done       integer     NOT NULL DEFAULT 0 CHECK (units_done BETWEEN 0 AND units_total),
    result           jsonb,
    created_at       timestamptz NOT NULL DEFAULT now(),
    finished_at      timestamptz
);

CREATE INDEX jobs_created_at ON jobs (created_at DESC);

CREATE TABLE units (
    id         uuid PRIMARY KEY,
    job_id     uuid        NOT NULL REFERENCES jobs (id) ON DELETE CASCADE,
    seq        integer     NOT NULL,
    params     jsonb       NOT NULL,
    status     text        NOT NULL CHECK (status IN ('pending', 'leased', 'done')),
    -- The accepted result (verification will choose among lease outputs).
    output     jsonb,
    created_at timestamptz NOT NULL,
    UNIQUE (job_id, seq)
);

-- Claim order: oldest job first, then unit sequence.
CREATE INDEX units_pending ON units (created_at, job_id, seq) WHERE status = 'pending';

CREATE TABLE workers (
    id              uuid PRIMARY KEY,
    backend         text        NOT NULL,
    vendor          text        NOT NULL,
    architecture    text        NOT NULL,
    client_version  text        NOT NULL,
    connected_at    timestamptz NOT NULL DEFAULT now(),
    last_seen_at    timestamptz NOT NULL DEFAULT now(),
    disconnected_at timestamptz
);

-- One row per assignment of a unit to a worker. Separate from units because
-- redundant execution assigns the same unit to several workers.
CREATE TABLE leases (
    id           uuid PRIMARY KEY,
    unit_id      uuid        NOT NULL REFERENCES units (id) ON DELETE CASCADE,
    worker_id    uuid        NOT NULL REFERENCES workers (id),
    issued_at    timestamptz NOT NULL DEFAULT now(),
    expires_at   timestamptz NOT NULL,
    completed_at timestamptz,
    released_at  timestamptz,
    output       jsonb,
    elapsed_ms   integer,
    CHECK (completed_at IS NULL OR released_at IS NULL)
);

CREATE INDEX leases_active_by_worker ON leases (worker_id)
    WHERE completed_at IS NULL AND released_at IS NULL;
CREATE INDEX leases_unit ON leases (unit_id);
