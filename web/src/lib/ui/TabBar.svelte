<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { account } from '#lib/account.svelte.ts';
	import Icon from '#lib/ui/Icon.svelte';

	// The phone's navigation (docs/design/design-system.md, „Рояк в джоба“): three places, the
	// middle one the product's single action, raised where the thumb rests. Hidden from
	// 761 px up, where the header carries the navigation.

	const path = $derived(page.url.pathname);
	const home = $derived(path === resolve('/'));
	const join = $derived(path.startsWith(resolve('/worker')) || path.startsWith(resolve('/join')));
	const signedIn = $derived(account.me !== null);
	const accountHere = $derived(
		['/app', '/login', '/invite'].some((p) => path.startsWith(p)) ? 'page' : undefined
	);
</script>

<nav class="tabbar" aria-label="Основни раздели">
	<a href={resolve('/')} aria-current={home ? 'page' : undefined} data-testid="tab-home">
		<Icon name="home" size={24} />
		<span>Начало</span>
	</a>
	<a
		class="join"
		href={resolve('/worker')}
		aria-current={join ? 'page' : undefined}
		data-testid="tab-join"
	>
		<span class="hex" aria-hidden="true">
			<svg viewBox="0 0 64 64">
				<path d="M32 4l24.25 14v28L32 60 7.75 46V18z" />
			</svg>
			<Icon name="power" size={26} />
		</span>
		<span>Включи се</span>
	</a>
	{#if signedIn}
		<a href={resolve('/app')} aria-current={accountHere} data-testid="tab-account">
			<Icon name="grid" size={24} />
			<span>Конзола</span>
		</a>
	{:else}
		<a href={resolve('/login')} aria-current={accountHere} data-testid="tab-account">
			<Icon name="user" size={24} />
			<span>Вход</span>
		</a>
	{/if}
</nav>

<style>
	.tabbar {
		display: none;
	}

	@media (max-width: 760px) {
		.tabbar {
			position: fixed;
			z-index: 60;
			left: max(12px, env(safe-area-inset-left));
			right: max(12px, env(safe-area-inset-right));
			bottom: max(10px, env(safe-area-inset-bottom));
			display: grid;
			grid-template-columns: repeat(3, minmax(0, 1fr));
			align-items: end;
			height: 64px;
			padding: 0 6px;
			border-radius: 24px;
			background: var(--glass-strong);
			border: 1px solid var(--glass-border);
			box-shadow:
				inset 0 1px 0 var(--glass-highlight),
				0 10px 30px -10px rgb(0 0 0 / 0.28);
			-webkit-backdrop-filter: blur(var(--glass-blur)) saturate(170%);
			backdrop-filter: blur(var(--glass-blur)) saturate(170%);
		}
	}

	@media (prefers-reduced-transparency: reduce) {
		.tabbar {
			background: var(--bg-elevated);
		}
	}

	a {
		display: grid;
		justify-items: center;
		align-content: center;
		gap: 3px;
		height: 64px;
		color: var(--muted);
		text-decoration: none;
		font-size: 0.6875rem;
		font-weight: 600;
		letter-spacing: 0.01em;
		-webkit-tap-highlight-color: transparent;
		transition:
			color var(--fast) var(--ease),
			transform var(--press) var(--ease);
	}
	a:active {
		transform: scale(0.94);
	}
	a[aria-current='page'] {
		color: var(--brand-ink);
	}
	a:focus-visible {
		outline-offset: -4px;
		border-radius: 18px;
	}

	/* The product's one action: a raised hexagon, like a cell of the logo. */
	.join {
		align-content: end;
		padding-bottom: 9px;
		color: var(--text);
	}
	.hex {
		position: relative;
		display: grid;
		place-items: center;
		width: 60px;
		height: 60px;
		margin-top: -26px;
		color: var(--on-brand);
	}
	.hex svg {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		overflow: visible;
		filter: drop-shadow(0 8px 14px rgb(224 142 11 / 0.45));
	}
	.hex path {
		fill: var(--brand);
		stroke: var(--brand);
		stroke-width: 6;
		stroke-linejoin: round;
	}
	/* The glyph sits over the hexagon drawn behind it. */
	.hex > :global(svg:last-child) {
		position: relative;
	}
	.join[aria-current='page'] {
		color: var(--brand-ink);
	}
</style>
