import { getContext, setContext } from 'svelte';
import { api } from '#lib/api.ts';
import type { NetworkView } from '#lib/protocol/NetworkView.ts';
import type { Role } from '#lib/protocol/Role.ts';

/** The signed-in account's networks, for the console's sidebar (spec 0010). */
class Networks {
	list = $state<NetworkView[]>([]);
	loaded = $state(false);

	async reload() {
		this.list = await api<NetworkView[]>('/api/networks');
		this.loaded = true;
	}
}

export const networks = new Networks();

const RANK: Record<Role, number> = { viewer: 0, admin: 1, owner: 2 };

/** Whether `role` is at least `min`. */
export const can = (role: Role, min: Role) => RANK[role] >= RANK[min];

/** The network a console page shows, shared by its tabs. */
export class CurrentNetwork {
	view = $state<NetworkView | null>(null);
	missing = $state(false);

	constructor(readonly id: string) {}

	async reload() {
		try {
			this.view = await api<NetworkView>(`/api/networks/${this.id}`);
		} catch {
			this.missing = true;
		}
	}

	/** Replaces the view after the API returned a newer one. */
	set(view: NetworkView) {
		this.view = view;
		const i = networks.list.findIndex((n) => n.id === view.id);
		if (i >= 0) networks.list[i] = view;
	}
}

const KEY = Symbol('network');
export const provideNetwork = (n: CurrentNetwork) => setContext(KEY, n);
export const useNetwork = () => getContext<CurrentNetwork>(KEY);
