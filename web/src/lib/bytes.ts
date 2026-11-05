// Bytes as they travel in protocol messages and through the API.

/** Base64 of `bytes`, in chunks so large inputs do not overflow the call stack. */
export function toBase64(bytes: Uint8Array): string {
	let binary = '';
	const CHUNK = 0x8000;
	for (let i = 0; i < bytes.length; i += CHUNK) {
		binary += String.fromCharCode(...bytes.subarray(i, i + CHUNK));
	}
	return btoa(binary);
}

export function fromBase64(text: string): Uint8Array {
	const binary = atob(text);
	const bytes = new Uint8Array(binary.length);
	for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
	return bytes;
}

export function toHex(bytes: Uint8Array): string {
	return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
}

/** Bytes from hex; spaces are ignored. Throws on anything else. */
export function fromHex(text: string): Uint8Array {
	const hex = text.replace(/\s+/g, '');
	if (hex.length % 2 !== 0 || /[^0-9a-f]/i.test(hex)) throw new Error('not hex');
	return Uint8Array.from({ length: hex.length / 2 }, (_, i) =>
		parseInt(hex.slice(2 * i, 2 * i + 2), 16)
	);
}

/** Hex SHA-256 of `bytes`. */
export async function sha256OfBytes(bytes: Uint8Array): Promise<string> {
	return toHex(new Uint8Array(await crypto.subtle.digest('SHA-256', bytes as BufferSource)));
}
