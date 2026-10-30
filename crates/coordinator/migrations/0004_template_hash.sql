-- The hash of the kernel a job was created with. A verified unit is only a valid
-- honeypot for workers running that exact kernel: after a kernel change, honest
-- workers would fail old known answers and get banned. Jobs from before this
-- migration have no hash and are never used as honeypots.
ALTER TABLE jobs ADD COLUMN template_sha256 text;
