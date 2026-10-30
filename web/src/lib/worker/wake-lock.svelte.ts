/**
 * Keeps the screen on while `wanted` is true (Screen Wake Lock API). A phone whose
 * screen turns off hides the page, the browser throttles it, and the device quietly
 * stops computing. The browser releases the lock whenever the page is hidden, so it is
 * taken again on return.
 */
export class ScreenAwake {
	/** True while the screen is held on. */
	held = $state(false);
	readonly supported = typeof navigator !== 'undefined' && 'wakeLock' in navigator;

	private sentinel: WakeLockSentinel | null = null;
	private wanted = false;

	constructor() {
		if (typeof document !== 'undefined') {
			document.addEventListener('visibilitychange', this.onVisibility);
		}
	}

	async set(wanted: boolean) {
		this.wanted = wanted;
		if (wanted) await this.acquire();
		else await this.release();
	}

	destroy() {
		this.wanted = false;
		document.removeEventListener('visibilitychange', this.onVisibility);
		void this.release();
	}

	private onVisibility = () => {
		if (this.wanted && document.visibilityState === 'visible') void this.acquire();
	};

	private async acquire() {
		if (!this.supported || this.sentinel || document.visibilityState !== 'visible') return;
		try {
			const sentinel = await navigator.wakeLock.request('screen');
			if (!this.wanted) {
				await sentinel.release();
				return;
			}
			this.sentinel = sentinel;
			this.held = true;
			sentinel.addEventListener('release', () => {
				if (this.sentinel === sentinel) this.sentinel = null;
				this.held = false;
			});
		} catch {
			// Refused (battery saver, or no user gesture yet): computing goes on regardless.
		}
	}

	private async release() {
		const sentinel = this.sentinel;
		this.sentinel = null;
		this.held = false;
		await sentinel?.release();
	}
}
