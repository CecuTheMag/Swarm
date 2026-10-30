import { pauseReason, type BatteryState, type PauseReason } from './pacing.ts';

interface BatteryManager extends EventTarget {
	level: number;
	charging: boolean;
}

/**
 * Whether the page is visible and how the battery is doing (spec 0007). The Battery
 * Status API exists in Chromium browsers only; elsewhere the battery never pauses.
 */
export class DeviceConditions {
	hidden = $state(false);
	battery = $state<BatteryState | null>(null);
	readonly reason: PauseReason | null = $derived(pauseReason(this.hidden, this.battery));

	private manager: BatteryManager | null = null;
	private destroyed = false;

	constructor() {
		if (typeof document === 'undefined') return;
		this.hidden = document.visibilityState === 'hidden';
		document.addEventListener('visibilitychange', this.onVisibility);
		const nav = navigator as Navigator & { getBattery?: () => Promise<BatteryManager> };
		nav
			.getBattery?.()
			.then((manager) => {
				if (this.destroyed) return;
				this.manager = manager;
				manager.addEventListener('levelchange', this.onBattery);
				manager.addEventListener('chargingchange', this.onBattery);
				this.onBattery();
			})
			.catch(() => {});
	}

	destroy() {
		this.destroyed = true;
		document.removeEventListener('visibilitychange', this.onVisibility);
		this.manager?.removeEventListener('levelchange', this.onBattery);
		this.manager?.removeEventListener('chargingchange', this.onBattery);
	}

	private onVisibility = () => {
		this.hidden = document.visibilityState === 'hidden';
	};

	private onBattery = () => {
		if (this.manager) {
			this.battery = { level: this.manager.level, charging: this.manager.charging };
		}
	};
}
