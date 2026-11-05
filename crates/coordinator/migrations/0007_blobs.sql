-- Spec 0013: inputs that units refer to by their SHA-256 instead of carrying them (a
-- school's data), stored once and fetched by each device once.
CREATE TABLE blobs (
    sha256     text        PRIMARY KEY CHECK (sha256 ~ '^[0-9a-f]{64}$'),
    bytes      bytea       NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
