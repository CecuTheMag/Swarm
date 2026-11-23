<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { onMount } from 'svelte';
	import { api, failureText } from '#lib/api.ts';
	import { account } from '#lib/account.svelte.ts';
	import Logo from '#lib/ui/Logo.svelte';
	import Icon from '#lib/ui/Icon.svelte';
	import type { InviteInfo } from '#lib/protocol/InviteInfo.ts';
	import type { Role } from '#lib/protocol/Role.ts';

	// An invitation (spec 0010): it creates an account, and adds its holder to a network
	// when it names one. Someone already signed in only joins the network.
	const token = page.params.token!;
	let info = $state<InviteInfo | null>(null);
	let missing = $state(false);
	let name = $state('');
	let email = $state('');
	let password = $state('');
	let error = $state<string | null>(null);
	let busy = $state(false);

	const roleText: Record<Role, string> = {
		owner: 'собственик',
		admin: 'администратор',
		viewer: 'наблюдател'
	};

	onMount(async () => {
		try {
			info = await api<InviteInfo>(`/api/invites/${encodeURIComponent(token)}`);
			email = info.email ?? '';
		} catch {
			missing = true;
		}
		await account.check().catch(() => null);
	});

	async function accept(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		error = null;
		try {
			const res = await api<{ network_id: string | null }>(
				`/api/invites/${encodeURIComponent(token)}`,
				{ body: { email, name, password } }
			);
			await account.refresh();
			await goto(
				res.network_id ? resolve('/app/n/[id]', { id: res.network_id }) : resolve('/app'),
				{ replaceState: true }
			);
		} catch (e) {
			error = failureText(e);
		} finally {
			busy = false;
		}
	}
</script>

<svelte:head>
	<title>Покана · Рояк</title>
</svelte:head>

<main class="auth">
	{#if missing}
		<div class="card form">
			<span class="icon bad"><Icon name="alert" size={26} /></span>
			<h1>Поканата не е валидна</h1>
			<p class="muted">Вече е използвана или е изтекла. Помолете за нова.</p>
			<a class="button secondary" href={resolve('/login')}>Към входа</a>
		</div>
	{:else if info}
		<form class="card form" onsubmit={accept}>
			<Logo size={40} />
			<h1>
				{info.network_name ? `Покана за „${info.network_name}“` : 'Покана за Рояк'}
			</h1>
			<p class="muted">
				{info.invited_by ? `${info.invited_by} ви кани` : 'Поканени сте'}
				{info.network_name && info.role
					? ` като ${roleText[info.role]} на мрежата.`
					: ' да създадете профил.'}
			</p>
			{#if account.me}
				<p class="notice">Ще се присъедините като {account.me.name} ({account.me.email}).</p>
			{:else}
				<label>
					Име
					<input autocomplete="name" required bind:value={name} data-testid="invite-name" />
				</label>
				<label>
					Имейл
					<input
						type="email"
						autocomplete="email"
						required
						bind:value={email}
						data-testid="invite-email"
					/>
				</label>
				<label>
					Парола (поне 10 знака)
					<input
						type="password"
						autocomplete="new-password"
						minlength="10"
						required
						bind:value={password}
						data-testid="invite-password"
					/>
				</label>
			{/if}
			{#if error}<p class="notice bad" role="alert">{error}</p>{/if}
			<button type="submit" class="lg" disabled={busy}>
				{account.me ? 'Присъедини се' : 'Създай профил'}
			</button>
		</form>
	{/if}
</main>

<style>
	.auth {
		max-width: 460px;
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
	.icon {
		display: grid;
		place-items: center;
		width: 52px;
		height: 52px;
		border-radius: 50%;
	}
	.icon.bad {
		background: var(--bad-soft);
		color: var(--bad);
	}
</style>
