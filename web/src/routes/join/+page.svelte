<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import Logo from '#lib/ui/Logo.svelte';

	// For devices without a camera (lab PCs): type the network's code instead of
	// scanning its QR code (spec 0010).
	let code = $state('');
	let error = $state<string | null>(null);
	let busy = $state(false);

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		// People copy codes with spaces, dashes or capitals; the code has none of them.
		const clean = code.toLowerCase().replace(/[^a-z0-9]/g, '');
		if (!clean) return;
		busy = true;
		error = null;
		try {
			const res = await fetch(`/api/join/${encodeURIComponent(clean)}`);
			if (res.ok) await goto(resolve('/join/[code]', { code: clean }));
			else error = 'Няма мрежа с този код. Проверете го и опитайте пак.';
		} catch {
			error = 'Няма връзка със сървъра. Проверете интернета и опитайте пак.';
		} finally {
			busy = false;
		}
	}
</script>

<svelte:head>
	<title>Включи се с код · Рояк</title>
</svelte:head>

<main class="auth">
	<form class="card form" onsubmit={submit}>
		<Logo size={40} />
		<h1>Включи се с код</h1>
		<p class="muted">Въведете кода на мрежата, който ви е дал собственикът ѝ.</p>
		<label>
			Код
			<input
				bind:value={code}
				required
				autocomplete="off"
				autocapitalize="none"
				spellcheck="false"
				maxlength="40"
				placeholder="напр. 49m4jkjayp"
				data-testid="join-code-input"
			/>
		</label>
		{#if error}<p class="notice bad" role="alert">{error}</p>{/if}
		<button type="submit" class="lg" disabled={busy}>Продължи</button>
		<p class="muted small">
			Нямате код? <a href={resolve('/worker')}>Включете се в общата мрежа</a>.
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
