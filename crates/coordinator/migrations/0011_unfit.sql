-- Devices that cannot compute a job's units correctly (spec 0017: their answer to the
-- model's known question was wrong) get none of them again on that connection.
ALTER TABLE workers ADD COLUMN unfit_jobs uuid[] NOT NULL DEFAULT '{}';

ALTER TABLE leases DROP CONSTRAINT leases_release_reason_check;
ALTER TABLE leases ADD CONSTRAINT leases_release_reason_check
    CHECK (release_reason IN ('disconnected', 'failed', 'invalid', 'expired', 'unfit'));
