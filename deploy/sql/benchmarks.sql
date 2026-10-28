-- Per-device speed of completed π units, for docs/measurements.md.
-- Run with `just bench-report`.
SELECT w.vendor,
       w.architecture,
       w.backend,
       w.client_version,
       count(*)                                   AS units,
       round(avg(l.elapsed_ms))                   AS avg_unit_ms,
       round(sum((u.params ->> 'samples')::numeric)
             / nullif(sum(l.elapsed_ms), 0) / 1000, 1) AS msamples_per_s,
       min(l.completed_at)::date                  AS first_seen
FROM leases l
JOIN workers w ON w.id = l.worker_id
JOIN units u ON u.id = l.unit_id
JOIN jobs j ON j.id = u.job_id
WHERE l.completed_at IS NOT NULL
  AND j.template = 'pi'
GROUP BY w.vendor, w.architecture, w.backend, w.client_version
ORDER BY msamples_per_s DESC NULLS LAST;
