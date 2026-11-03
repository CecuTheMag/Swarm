<script lang="ts">
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { api, failureText } from '#lib/api.ts';
	import { account } from '#lib/account.svelte.ts';
	import CopyField from '#lib/ui/CopyField.svelte';
	import type { AccountView } from '#lib/protocol/AccountView.ts';
	import type { CreatedInvite } from '#lib/protocol/CreatedInvite.ts';

	// The platform admin: who has an account, and invitations for new ones (spec 0010).
	let accounts = $state<AccountView[]>([]);
	let email = $state('');
	let link = $state<string | null>(null);
	let error = $state<string | null>(null);

	async function load() {
		try {
			accounts = await api<AccountView[]>('/api/admin/accounts');
		} catch (e) {
			error = failureText(e);
		}
	}
	$effect(() => {
		if (account.me?.is_admin) void load();
	});

	async function invite(event: SubmitEvent) {
		event.preventDefault();
		error = null;
		try {
			const created = await api<CreatedInvite>('/api/admin/invites', {
				body: { email: email || null }
			});
			link = new URL(resolve('/invite/[token]', { token: created.token }), page.url.origin).href;
		} catch (e) {
			error = failureText(e);
		}
	}
</script>

<svelte:head>
	<title>Администрация · Рояк</title>
</svelte:head>

<header class="page-head">
	<span class="eyebrow">Администрация</span>
	<h1>Профили</h1>
	<p>Профили се създават само с покана. Изпратете връзката на човека; тя важи за един профил.</p>
</header>

{#if !account.me?.is_admin}
	<p class="notice bad">Тази страница е за администратора на платформата.</p>
{:else}
	{#if error}<p class="notice bad" role="alert">{error}</p>{/if}
	<section class="card">
		<h2>Покана за нов профил</h2>
		<form class="invite" onsubmit={invite}>
			<input type="email" bind:value={email} placeholder="Имейл (по желание)" aria-label="Имейл" />
			<button type="submit" data-testid="admin-invite">Създай покана</button>
		</form>
		{#if link}<CopyField value={link} label="Връзка за покана" testid="admin-invite-link" />{/if}
	</section>

	<section class="card">
		<h2>Всички профили ({accounts.length})</h2>
		<div class="table-wrap">
			<table>
				<thead><tr><th>Име</th><th>Имейл</th><th>Създаден</th><th>Последен вход</th></tr></thead>
				<tbody>
					{#each accounts as a (a.id)}
						<tr>
							<td>{a.name}{a.is_admin ? ' · админ' : ''}</td>
							<td>{a.email}</td>
							<td>{new Date(a.created_at).toLocaleDateString('bg-BG')}</td>
							<td class="muted">
								{a.last_login_at ? new Date(a.last_login_at).toLocaleString('bg-BG') : 'никога'}
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	</section>
{/if}

<style>
	section + section {
		margin-top: var(--s-4);
	}
	h2 {
		font-size: var(--text-lg);
	}
	.invite {
		display: flex;
		gap: var(--s-3);
		margin-bottom: var(--s-3);
	}
	@media (max-width: 560px) {
		.invite {
			flex-direction: column;
		}
	}
</style>
