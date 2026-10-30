// How hard a contributor's device works, and when it stops (spec 0007).

/** Share of the time the device computes, in percent. */
export const POWER_LEVELS = [25, 50, 75, 100] as const;
export type Power = (typeof POWER_LEVELS)[number];
export const DEFAULT_POWER: Power = 75;

/** Below this charge, a device that is not charging pauses. */
export const LOW_BATTERY = 0.3;

export type PauseReason = 'hidden' | 'battery';

export interface BatteryState {
	level: number;
	charging: boolean;
}

/** Rest after a unit that took `elapsedMs`, so the device computes `power` % of the time. */
export function restMs(elapsedMs: number, power: number): number {
	if (power >= 100) return 0;
	return (elapsedMs * (100 - power)) / power;
}

/** Why the device should not compute now, or null if it may. */
export function pauseReason(hidden: boolean, battery: BatteryState | null): PauseReason | null {
	if (hidden) return 'hidden';
	if (battery && !battery.charging && battery.level < LOW_BATTERY) return 'battery';
	return null;
}

const POWER_KEY = 'swarm-power';

/** The power level chosen on this browser before, or the default. */
export function loadPower(): Power {
	try {
		const saved = Number(localStorage.getItem(POWER_KEY));
		return POWER_LEVELS.find((p) => p === saved) ?? DEFAULT_POWER;
	} catch {
		return DEFAULT_POWER;
	}
}

export function savePower(power: Power) {
	try {
		localStorage.setItem(POWER_KEY, String(power));
	} catch {
		// Without storage the choice lasts until the page is closed.
	}
}
