// The coordinator under test requires a password for submitting jobs, as on the public
// server (SWARM_SUBMIT_TOKEN).
export const SUBMIT_PASSWORD = 'e2e-submit-password';
export const SUBMIT_HEADERS = { authorization: `Bearer ${SUBMIT_PASSWORD}` };
