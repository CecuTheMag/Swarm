<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { onMount } from 'svelte';
	import { api, failureText } from '#lib/api.ts';
	import { account } from '#lib/account.svelte.ts';
	import Logo from '#lib/ui/Logo.svelte';

	let email = $state('');
	let password = $state('');
	let error = $state<string | null>(null);
	let busy = $state(false);

	/** Where to go after signing in: only paths inside the console. */
	const next = $derived.by(() => {
		const n = page.url.searchParams.get('next');
		return n && n.startsWith('/app') ? n : resolve('/app');
	});

	onMount(async () => {
		// eslint-disable-next-line svelte/no-navigation-without-resolve -- `next` is a resolved console path
		if (await account.check().catch(() => null)) await goto(next, { replaceState: true });
	});

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		error = null;
		try {
			await api('/api/auth/login', { body: { email, password } });
			await account.refresh();
			// eslint-disable-next-line svelte/no-navigation-without-resolve -- `next` is a resolved console path
			await goto(next, { replaceState: true });
		} catch (e) {
			error =
				failureText(e) === 'wrong email or password' ? 'Грешен имейл или парола.' : failureText(e);
		} finally {
			busy = false;
		}
	}
</script>

<svelte:head>
	<title>Вход · Рояк</title>
</svelte:head>

<main class="auth">
	<form class="card form" onsubmit={submit}>
		<Logo size={40} />
		<h1>Вход</h1>
		<p class="muted">Влезте, за да управлявате своите мрежи от устройства.</p>
		<label>
			Имейл
			<input
				type="email"
				autocomplete="email"
				required
				bind:value={email}
				data-testid="login-email"
			/>
		</label>
		<label>
			Парола
			<input
				type="password"
				autocomplete="current-password"
				required
				bind:value={password}
				data-testid="login-password"
			/>
		</label>
		{#if error}<p class="notice bad" role="alert">{error}</p>{/if}
		<button type="submit" class="lg" disabled={busy}>Влез</button>
		<p class="muted small">
			Достъпът е с покана. Ако организацията ви вече ползва Рояк, помолете администратора ѝ за
			покана.
		</p>
	</form>
</main>

<style>
	.auth {
		max-width: 440px;
		padding-top: var(--s-7);
	}
	.form {
		display: grid;
		gap: var(--s-4);
		padding: var(--s-6) var(--s-5);
	}
	.form h1 {
		margin: var(--s-2) 0 0;
		font-size: var(--text-xl);
	}
	.form > p {
		margin: 0;
	}
	.small {
		font-size: var(--text-xs);
	}
</style>
