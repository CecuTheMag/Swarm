import { api, ApiFailure } from '#lib/api.ts';
import type { Me } from '#lib/protocol/Me.ts';

/** The signed-in account, shared by the header and the console (spec 0010). */
class Account {
	me = $state<Me | null>(null);
	/** True once the first check finished, signed in or not. */
	known = $state(false);

	async refresh(): Promise<Me | null> {
		try {
			this.me = await api<Me>('/api/me');
		} catch (e) {
			if (!(e instanceof ApiFailure && e.status === 401)) throw e;
			this.me = null;
		} finally {
			this.known = true;
		}
		return this.me;
	}

	async logout() {
		await api('/api/auth/logout', { body: {} });
		this.me = null;
	}
}

export const account = new Account();
