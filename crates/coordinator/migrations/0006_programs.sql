-- Spec 0012: programs, WebAssembly modules a network uploads and runs.

-- Modules by content: the same bytes uploaded twice are stored once.
CREATE TABLE modules (
    sha256     text        PRIMARY KEY CHECK (sha256 ~ '^[0-9a-f]{64}$'),
    bytes      bytea       NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE programs (
    id          uuid        PRIMARY KEY,
    network_id  uuid        NOT NULL REFERENCES networks (id) ON DELETE CASCADE,
    sha256      text        NOT NULL REFERENCES modules (sha256),
    name        text        NOT NULL CHECK (length(name) BETWEEN 1 AND 80),
    description text        NOT NULL DEFAULT '' CHECK (length(description) <= 2000),
    source_url  text        CHECK (length(source_url) <= 300),
    -- private: runs only in its network. pending: review was asked for. approved: may
    -- run on the public network. rejected: may not, with a note why.
    review      text        NOT NULL DEFAULT 'private'
                            CHECK (review IN ('private', 'pending', 'approved', 'rejected')),
    review_note text        CHECK (length(review_note) <= 2000),
    reviewed_at timestamptz,
    created_at  timestamptz NOT NULL DEFAULT now(),
    UNIQUE (network_id, sha256)
);

CREATE INDEX programs_review ON programs (review, created_at) WHERE review IN ('pending', 'approved');

ALTER TABLE jobs ADD COLUMN program_id uuid REFERENCES programs (id) ON DELETE SET NULL;
-- The network that submitted a job running on the public network; it stays theirs.
ALTER TABLE jobs ADD COLUMN owner_network_id uuid REFERENCES networks (id) ON DELETE SET NULL;
CREATE INDEX jobs_owner_network ON jobs (owner_network_id, created_at DESC)
    WHERE owner_network_id IS NOT NULL;

-- Agreeing results each unit needs (spec 0004). It was the coordinator's setting for
-- every job; a network may now choose it per job. A unit's target starts at it and only
-- grows, so a job's smallest target is the setting it was created with.
ALTER TABLE jobs ADD COLUMN quorum integer;
UPDATE jobs SET quorum = (SELECT coalesce(min(target), 1) FROM units WHERE job_id = jobs.id);
ALTER TABLE jobs ALTER COLUMN quorum SET NOT NULL;
ALTER TABLE jobs ADD CONSTRAINT jobs_quorum_positive CHECK (quorum >= 1);

-- Whether the device also runs reviewed community programs on the public network.
ALTER TABLE workers ADD COLUMN community boolean NOT NULL DEFAULT true;
