<script lang="ts" module>
	// Line icons on a 24 px grid, drawn with the current text colour.
	const paths = {
		arrow: 'M5 12h14M13 6l6 6-6 6',
		power: 'M12 3v8M6.3 6.6a8 8 0 1 0 11.4 0',
		shield: 'M12 3l7 3v5c0 4.6-3 8.3-7 10-4-1.7-7-5.4-7-10V6z M9 12l2 2 4-4',
		battery: 'M3 8h15v8H3z M21 11v2 M6 11v2',
		eye: 'M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12z M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6z',
		chip: 'M7 7h10v10H7z M10 3v4M14 3v4M10 17v4M14 17v4M3 10h4M3 14h4M17 10h4M17 14h4',
		image: 'M4 5h16v14H4z M4 16l5-5 4 4 2-2 5 5 M15.5 9.5a1.5 1.5 0 1 0 0-.01',
		atom: 'M12 12m-1.5 0a1.5 1.5 0 1 0 3 0 1.5 1.5 0 1 0-3 0 M12 3c4.4 0 4.4 18 0 18S7.6 3 12 3z M4.2 7.5c2.2-3.8 17.8 5.2 15.6 9S2 11.3 4.2 7.5z',
		school: 'M3 9l9-5 9 5-9 5z M7 11.5V16c3 2 7 2 10 0v-4.5 M21 9v6',
		check: 'M5 12.5l4.5 4.5L19 7',
		close: 'M6 6l12 12M18 6L6 18',
		menu: 'M4 8h16M4 16h16',
		lock: 'M6 11h12v9H6z M8.5 11V8a3.5 3.5 0 0 1 7 0v3',
		phone: 'M8 3h8a1 1 0 0 1 1 1v16a1 1 0 0 1-1 1H8a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1z M11 18h2',
		globe:
			'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18z M3 12h18 M12 3c2.5 2.5 3.8 5.5 3.8 9s-1.3 6.5-3.8 9c-2.5-2.5-3.8-5.5-3.8-9S9.5 5.5 12 3z',
		pause: 'M9 6v12M15 6v12',
		info: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18z M12 11v5 M12 7.5v.5',
		alert: 'M12 4l9 16H3z M12 10v4 M12 17v.5',
		bolt: 'M13 3L5 13h6l-1 8 8-10h-6z',
		code: 'M8 7l-5 5 5 5M16 7l5 5-5 5'
	} as const;
	export type IconName = keyof typeof paths;
</script>

<script lang="ts">
	let { name, size = 20, label }: { name: IconName; size?: number; label?: string } = $props();
</script>

<svg
	width={size}
	height={size}
	viewBox="0 0 24 24"
	fill="none"
	stroke="currentColor"
	stroke-width="1.7"
	stroke-linecap="round"
	stroke-linejoin="round"
	role={label ? 'img' : undefined}
	aria-label={label}
	aria-hidden={label ? undefined : 'true'}
>
	<path d={paths[name]} />
</svg>
