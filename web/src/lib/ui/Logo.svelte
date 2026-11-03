<script lang="ts">
	// The mark: three cells of a honeycomb, each a device.
	let { size = 28 }: { size?: number } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 64 64" aria-hidden="true" class="mark">
	<path d="M32 6l11 6.4v12.7L32 31.5l-11-6.4V12.4z" class="c1" />
	<path d="M18.7 30.5l11 6.4v12.7l-11 6.4-11-6.4V36.9z" class="c2" />
	<path d="M45.3 30.5l11 6.4v12.7l-11 6.4-11-6.4V36.9z" class="c3" />
</svg>

<style>
	.mark path {
		fill: var(--brand);
		transition: transform var(--slow) var(--ease);
		transform-origin: center;
		transform-box: fill-box;
	}
	.c2 {
		opacity: 0.78;
	}
	.c3 {
		opacity: 0.55;
	}
	:global(a:hover) > .mark .c1 {
		transform: translateY(-2px);
	}
	:global(a:hover) > .mark .c3 {
		transform: translateX(1px);
	}
</style>
