-- Spec 0015: a found timetable as a person edited it, one per job. `version` grows with
-- every save, so a save made from an older version is refused.
CREATE TABLE timetable_edits (
    job_id     uuid PRIMARY KEY REFERENCES jobs (id) ON DELETE CASCADE,
    slots      text        NOT NULL,
    pinned     integer[]   NOT NULL,
    cost       jsonb       NOT NULL,
    version    integer     NOT NULL DEFAULT 1 CHECK (version >= 1),
    updated_at timestamptz NOT NULL DEFAULT now(),
    updated_by uuid        REFERENCES accounts (id) ON DELETE SET NULL
);
