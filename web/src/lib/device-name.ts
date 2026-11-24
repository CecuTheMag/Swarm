// How a device is named on screen. Some browsers hide the graphics card from pages
// (Brave's fingerprinting protection returns blank vendor and architecture), and the
// device then reports "unknown": it still computes, only the model is not known.

import type { DeviceInfo } from '#lib/protocol/DeviceInfo.ts';

/** What a device reports for a graphics card its browser keeps hidden. */
export const HIDDEN = 'unknown';

const HIDDEN_TEXT = 'скрита от браузъра';

const blank = (s: string | undefined) => !s || s.trim() === '' || s.trim() === HIDDEN;

/**
 * The system a browser runs on, from its user-agent string: the one thing left to tell a
 * phone from a laptop when the graphics card is hidden. `undefined` when unrecognised.
 */
export function platformName(userAgent: string): string | undefined {
	if (/Android/i.test(userAgent)) return 'Android';
	if (/iPhone|iPad|iPod/i.test(userAgent)) return 'iOS';
	if (/Windows/i.test(userAgent)) return 'Windows';
	if (/CrOS/i.test(userAgent)) return 'ChromeOS';
	if (/Mac OS X|Macintosh/i.test(userAgent)) return 'macOS';
	if (/Linux|X11/i.test(userAgent)) return 'Linux';
	return undefined;
}

/**
 * The vendor and architecture a device reports for its graphics card: the adapter's own
 * words, or `unknown` with the system in brackets when the browser hides them.
 */
export function reportedGpu(
	info: { vendor?: string; architecture?: string; description?: string },
	userAgent: string
): Pick<DeviceInfo, 'vendor' | 'architecture'> {
	const vendor = info.vendor?.trim() || HIDDEN;
	const architecture = info.architecture?.trim() || info.description?.trim();
	if (architecture) return { vendor, architecture };
	if (vendor !== HIDDEN) return { vendor, architecture: HIDDEN };
	const platform = platformName(userAgent);
	return { vendor, architecture: platform ? `${HIDDEN} (${platform})` : HIDDEN };
}

/** A graphics card as people read it: its name, or that the browser hides it. */
export function gpuName(vendor: string, architecture: string): string {
	const hiddenOn = /^unknown \((.+)\)$/.exec(architecture.trim())?.[1];
	if (blank(vendor) && (blank(architecture) || hiddenOn)) {
		return hiddenOn ? `${HIDDEN_TEXT} · ${hiddenOn}` : HIDDEN_TEXT;
	}
	if (blank(vendor)) return architecture;
	if (blank(architecture)) return vendor;
	return `${vendor} · ${architecture}`;
}

/** What a device computes with: a graphics card's name, or the CPU's thread count. */
export function deviceDetail(device: DeviceInfo): string {
	return device.backend === 'webgpu'
		? gpuName(device.vendor, device.architecture)
		: `${device.vendor} · ${device.architecture}`;
}
