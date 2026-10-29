-- Spec 0004: results are verified by agreement between clients, known-answer units
-- and reputation.

-- A device across connections, identified by the id its browser keeps.
CREATE TABLE clients (
    id         uuid PRIMARY KEY,
    created_at timestamptz NOT NULL DEFAULT now(),
    correct    integer     NOT NULL DEFAULT 0,
    incorrect  integer     NOT NULL DEFAULT 0,
    -- Wrong answers to known-answer units: proof, where disagreement is only evidence.
    honeypot_failures integer NOT NULL DEFAULT 0,
    banned_at  timestamptz
);

ALTER TABLE workers ADD COLUMN client_id uuid REFERENCES clients (id);
CREATE INDEX workers_client ON workers (client_id);

-- Results to collect before deciding; grows by one after each disagreement.
ALTER TABLE units ADD COLUMN target integer NOT NULL DEFAULT 1 CHECK (target >= 1);

ALTER TABLE leases
    -- A copy of an already verified unit, checked against its known output.
    ADD COLUMN honeypot boolean NOT NULL DEFAULT false,
    -- Given to a trusted client without a second copy.
    ADD COLUMN solo boolean NOT NULL DEFAULT false,
    -- How the result compared once the unit was decided.
    ADD COLUMN verdict text
        CHECK (verdict IN ('agreed', 'disagreed', 'trusted', 'honeypot_ok', 'honeypot_bad'));

-- Every completed result of a unit, for deciding it.
CREATE INDEX leases_completed_by_unit ON leases (unit_id) WHERE completed_at IS NOT NULL;
