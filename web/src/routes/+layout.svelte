<script lang="ts">
	import { resolve } from '$app/paths';
	import '../app.css';
	import favicon from '#lib/assets/favicon.svg';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
	<title>Рояк</title>
</svelte:head>

<header>
	<nav>
		<a class="brand" href={resolve('/')}>Рояк</a>
		<a href={resolve('/worker')}>Включи се</a>
		<a href={resolve('/dashboard')}>Табло</a>
		<a href={resolve('/selftest')}>Самопроверка</a>
	</nav>
</header>

{@render children()}

<style>
	header {
		border-bottom: 1px solid var(--border);
		background: var(--surface);
	}
	nav {
		max-width: 1080px;
		margin: 0 auto;
		padding: 12px 16px;
		display: flex;
		gap: 20px;
		align-items: center;
		flex-wrap: wrap;
	}
	nav a {
		color: var(--muted);
		text-decoration: none;
	}
	nav a:hover {
		color: var(--text);
	}
	.brand {
		color: var(--text);
		font-weight: 800;
		font-size: 1.15rem;
		margin-right: auto;
	}
</style>
