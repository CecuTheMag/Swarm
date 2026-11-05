// Whether this device runs reviewed community programs (spec 0012). Рояк's own
// templates always run; a network's own programs run on the devices that joined it.

const COMMUNITY_KEY = 'swarm-community';

export function loadCommunity(): boolean {
	try {
		return localStorage.getItem(COMMUNITY_KEY) !== 'no';
	} catch {
		return true;
	}
}

export function saveCommunity(allowed: boolean) {
	try {
		localStorage.setItem(COMMUNITY_KEY, allowed ? 'yes' : 'no');
	} catch {
		// Without storage the choice lasts until the page is closed.
	}
}
