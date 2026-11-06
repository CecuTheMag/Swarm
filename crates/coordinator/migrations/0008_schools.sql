-- Spec 0014: the schools a network keeps, as its deputy director entered them. `version`
-- grows with every save, so a save made from an older version is refused.
CREATE TABLE schools (
    id         uuid PRIMARY KEY,
    network_id uuid        NOT NULL REFERENCES networks (id) ON DELETE CASCADE,
    name       text        NOT NULL,
    data       jsonb       NOT NULL,
    version    integer     NOT NULL DEFAULT 1 CHECK (version >= 1),
    updated_at timestamptz NOT NULL DEFAULT now(),
    updated_by uuid        REFERENCES accounts (id) ON DELETE SET NULL
);
CREATE INDEX schools_network ON schools (network_id, updated_at DESC);
