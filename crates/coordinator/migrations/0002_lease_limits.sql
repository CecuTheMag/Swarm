-- Spec 0002: leases expire, a lease has a maximum lifetime, and a unit whose leases
-- keep ending without a result fails its job instead of cycling forever.

ALTER TABLE leases ADD COLUMN release_reason text
    CHECK (release_reason IN ('disconnected', 'failed', 'invalid', 'expired'));
-- Before this migration, leases were released only when their worker left.
UPDATE leases SET release_reason = 'disconnected' WHERE released_at IS NOT NULL;
ALTER TABLE leases ADD CONSTRAINT leases_release_reason_consistent
    CHECK ((released_at IS NULL) = (release_reason IS NULL));

-- The reaper's scan: active leases by expiry.
CREATE INDEX leases_active_by_expiry ON leases (expires_at)
    WHERE completed_at IS NULL AND released_at IS NULL;

-- Leases of the unit released without a result.
ALTER TABLE units ADD COLUMN releases integer NOT NULL DEFAULT 0;
ALTER TABLE units DROP CONSTRAINT units_status_check;
ALTER TABLE units ADD CONSTRAINT units_status_check
    CHECK (status IN ('pending', 'leased', 'done', 'failed'));

ALTER TABLE jobs ADD COLUMN error text;
