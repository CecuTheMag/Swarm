/**
 * The organiser's submit password (SWARM_SUBMIT_TOKEN), kept on this device so it is
 * typed once on the dashboard. Starting and stopping jobs and the chaos panel need it.
 */
const PASSWORD_KEY = 'swarm-submit-password';

export function loadPassword(): string {
	try {
		return localStorage.getItem(PASSWORD_KEY) ?? '';
	} catch {
		// Storage may be unavailable (private mode).
		return '';
	}
}

export function savePassword(password: string) {
	try {
		localStorage.setItem(PASSWORD_KEY, password);
	} catch {
		// Not remembered; nothing else depends on it.
	}
}

/** Request headers carrying the password, if there is one. */
export function authHeaders(password: string): Record<string, string> {
	return password ? { authorization: `Bearer ${password}` } : {};
}
