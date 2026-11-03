<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import WorkerConsole from '#lib/worker/WorkerConsole.svelte';
	import Icon from '#lib/ui/Icon.svelte';

	// A device joins a client's network from its link or QR code (spec 0010).
	const code = page.params.code!;
	let phase = $state<'checking' | 'ok' | 'missing'>('checking');
	let name = $state('');
	onMount(async () => {
		const res = await fetch(`/api/join/${encodeURIComponent(code)}`);
		if (res.ok) {
			name = ((await res.json()) as { name: string }).name;
			phase = 'ok';
		} else {
			phase = 'missing';
		}
	});
</script>

<svelte:head>
	<title>{name ? `${name} · Рояк` : 'Включи се · Рояк'}</title>
</svelte:head>

{#if phase === 'missing'}
	<main class="missing">
		<div class="card">
			<span class="icon"><Icon name="alert" size={28} /></span>
			<h1>Връзката не е валидна</h1>
			<p class="muted">
				Мрежата не съществува или собственикът ѝ е сменил връзката за включване. Помолете го за
				нова.
			</p>
			<a class="button secondary" href={resolve('/worker')}>Включи се в общата мрежа</a>
		</div>
	</main>
{:else if phase === 'ok'}
	<WorkerConsole joinCode={code} />
{/if}

<style>
	.missing {
		max-width: 560px;
	}
	.missing .card {
		display: grid;
		justify-items: center;
		text-align: center;
		gap: var(--s-3);
		padding: var(--s-7) var(--s-5);
	}
	.icon {
		display: grid;
		place-items: center;
		width: 56px;
		height: 56px;
		border-radius: 50%;
		background: var(--bad-soft);
		color: var(--bad);
	}
</style>
