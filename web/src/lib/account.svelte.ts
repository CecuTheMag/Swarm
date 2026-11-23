import { api, ApiFailure } from '#lib/api.ts';
import type { Me } from '#lib/protocol/Me.ts';

/** Set once this browser has signed in, cleared when the server says it is not. */
const HINT = 'swarm.signed-in';

function hinted(): boolean {
	try {
		return localStorage.getItem(HINT) === '1';
	} catch {
		// No storage (a private window, say): ask the server as before.
		return true;
	}
}

function setHint(on: boolean) {
	try {
		if (on) localStorage.setItem(HINT, '1');
		else localStorage.removeItem(HINT);
	} catch {
		// Without storage the hint is simply not kept.
	}
}

/** The signed-in account, shared by the header and the console (spec 0010). */
class Account {
	me = $state<Me | null>(null);
	/** True once the first check finished, signed in or not. */
	known = $state(false);

	async refresh(): Promise<Me | null> {
		try {
			this.me = await api<Me>('/api/me');
			setHint(true);
		} catch (e) {
			if (!(e instanceof ApiFailure && e.status === 401)) throw e;
			this.me = null;
			setHint(false);
		} finally {
			this.known = true;
		}
		return this.me;
	}

	/**
	 * As refresh(), but on public pages: asks the server only if this browser has signed
	 * in before, so a visitor who never has does not get a failed request (and a console
	 * error) on every page. The console itself always asks (refresh()).
	 */
	async check(): Promise<Me | null> {
		if (hinted()) return this.refresh();
		this.me = null;
		this.known = true;
		return null;
	}

	async logout() {
		await api('/api/auth/logout', { body: {} });
		this.me = null;
		setHint(false);
	}
}

export const account = new Account();
