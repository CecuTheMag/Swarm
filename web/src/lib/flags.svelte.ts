import { api } from '#lib/api.ts';

/**
 * Feature flags (spec 0020), read once per page load. A flag counts as on until the
 * server says otherwise: the server refuses what is off, so the console only hides it.
 */
class Flags {
	states = $state<Record<string, boolean>>({});
	#loading: Promise<void> | null = null;

	on(key: string): boolean {
		return this.states[key] !== false;
	}

	load(): Promise<void> {
		this.#loading ??= api<Record<string, boolean>>('/api/flags')
			.then((s) => {
				this.states = s;
			})
			.catch(() => {
				// Unknown flags stay on; the server still refuses what is off.
				this.#loading = null;
			});
		return this.#loading;
	}

	/** After the admin switches one, so this page shows the change at once. */
	set(key: string, enabled: boolean) {
		this.states = { ...this.states, [key]: enabled };
	}
}

export const flags = new Flags();
