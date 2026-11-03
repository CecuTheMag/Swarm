-- Networks of devices that clients own and operate (spec 0010): accounts and sessions,
-- invitations, networks with their rules, members, devices and API keys. Everything that
-- existed before belongs to the public network.

CREATE TABLE accounts (
    id            uuid PRIMARY KEY,
    email         text        NOT NULL,
    name          text        NOT NULL,
    password_hash text        NOT NULL,
    -- The platform's operator: invites accounts and runs the public network.
    is_admin      boolean     NOT NULL DEFAULT false,
    created_at    timestamptz NOT NULL DEFAULT now(),
    last_login_at timestamptz
);
CREATE UNIQUE INDEX accounts_email ON accounts (lower(email));

-- Only a hash of each session token is stored; the token lives in an HttpOnly cookie.
CREATE TABLE sessions (
    token_hash bytea PRIMARY KEY,
    account_id uuid        NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL
);
CREATE INDEX sessions_account ON sessions (account_id);

CREATE TABLE networks (
    id         uuid PRIMARY KEY,
    name       text        NOT NULL,
    -- In the join link and QR code; whoever has it can add a device.
    join_code  text        NOT NULL UNIQUE,
    join_mode  text        NOT NULL DEFAULT 'open' CHECK (join_mode IN ('open', 'approval')),
    max_power  integer     NOT NULL DEFAULT 100 CHECK (max_power IN (25, 50, 75, 100)),
    -- NULL: always. Otherwise {"timezone": "Europe/Sofia", "windows": [{"days": [1..7],
    -- "from": "HH:MM", "to": "HH:MM"}]} (see hours.rs).
    hours      jsonb,
    paused     boolean     NOT NULL DEFAULT false,
    -- Idle devices help the public network when this one has nothing to do.
    lend_idle  boolean     NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT now()
);
INSERT INTO networks (id, name, join_code)
VALUES ('00000000-0000-0000-0000-000000000001', 'Общата мрежа', 'public');

CREATE TABLE memberships (
    network_id uuid        NOT NULL REFERENCES networks (id) ON DELETE CASCADE,
    account_id uuid        NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    role       text        NOT NULL CHECK (role IN ('owner', 'admin', 'viewer')),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (network_id, account_id)
);
CREATE INDEX memberships_account ON memberships (account_id);

-- An invitation creates an account (accounts exist only by invitation) and, when it
-- names a network, makes its holder a member.
CREATE TABLE invites (
    token_hash bytea PRIMARY KEY,
    email      text,
    network_id uuid        REFERENCES networks (id) ON DELETE CASCADE,
    role       text        CHECK (role IN ('owner', 'admin', 'viewer')),
    created_by uuid        REFERENCES accounts (id) ON DELETE SET NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    used_at    timestamptz,
    used_by    uuid        REFERENCES accounts (id) ON DELETE SET NULL,
    CHECK ((network_id IS NULL) = (role IS NULL))
);

-- A browser (client) in a network. Its reputation stays on `clients`, across networks.
CREATE TABLE devices (
    id            uuid PRIMARY KEY,
    network_id    uuid        NOT NULL REFERENCES networks (id) ON DELETE CASCADE,
    client_id     uuid        NOT NULL REFERENCES clients (id),
    name          text,
    status        text        NOT NULL DEFAULT 'active'
                              CHECK (status IN ('active', 'pending', 'removed')),
    paused        boolean     NOT NULL DEFAULT false,
    backend       text        NOT NULL,
    vendor        text        NOT NULL,
    architecture  text        NOT NULL,
    units_done    integer     NOT NULL DEFAULT 0,
    first_seen_at timestamptz NOT NULL DEFAULT now(),
    last_seen_at  timestamptz NOT NULL DEFAULT now(),
    UNIQUE (network_id, client_id)
);

CREATE TABLE api_keys (
    id           uuid PRIMARY KEY,
    network_id   uuid        NOT NULL REFERENCES networks (id) ON DELETE CASCADE,
    token_hash   bytea       NOT NULL UNIQUE,
    label        text        NOT NULL,
    created_by   uuid        REFERENCES accounts (id) ON DELETE SET NULL,
    created_at   timestamptz NOT NULL DEFAULT now(),
    last_used_at timestamptz,
    revoked_at   timestamptz
);
CREATE INDEX api_keys_network ON api_keys (network_id);

ALTER TABLE jobs ADD COLUMN network_id uuid NOT NULL
    DEFAULT '00000000-0000-0000-0000-000000000001' REFERENCES networks (id) ON DELETE CASCADE;
ALTER TABLE jobs ALTER COLUMN network_id DROP DEFAULT;
CREATE INDEX jobs_network ON jobs (network_id, created_at DESC);

-- Copied from the job, so a device's work comes straight off this index.
ALTER TABLE units ADD COLUMN network_id uuid;
UPDATE units u SET network_id = j.network_id FROM jobs j WHERE j.id = u.job_id;
ALTER TABLE units ALTER COLUMN network_id SET NOT NULL;
DROP INDEX units_pending;
CREATE INDEX units_pending ON units (network_id, created_at, job_id, seq) WHERE status = 'pending';

ALTER TABLE workers
    ADD COLUMN network_id uuid REFERENCES networks (id) ON DELETE SET NULL,
    ADD COLUMN device_id  uuid REFERENCES devices (id) ON DELETE SET NULL;
