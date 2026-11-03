<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { api, failureText } from '#lib/api.ts';
	import { account } from '#lib/account.svelte.ts';
	import { can, networks, useNetwork } from '#lib/console/networks.svelte.ts';
	import ConfirmButton from '#lib/ui/ConfirmButton.svelte';
	import CopyField from '#lib/ui/CopyField.svelte';
	import type { CreatedInvite } from '#lib/protocol/CreatedInvite.ts';
	import type { MemberView } from '#lib/protocol/MemberView.ts';
	import type { Role } from '#lib/protocol/Role.ts';

	const network = useNetwork();
	const id = $derived(page.params.id!);
	const view = $derived(network.view!);
	const owner = $derived(can(view.role, 'owner'));
	const admin = $derived(can(view.role, 'admin'));

	let members = $state<MemberView[]>([]);
	let error = $state<string | null>(null);
	let role = $state<Role>('viewer');
	let email = $state('');
	let link = $state<string | null>(null);
	let expires = $state<string | null>(null);

	async function load() {
		try {
			members = await api<MemberView[]>(`/api/networks/${id}/members`);
		} catch (e) {
			error = failureText(e);
		}
	}
	$effect(() => {
		void load();
	});

	async function invite(event: SubmitEvent) {
		event.preventDefault();
		error = null;
		try {
			const created = await api<CreatedInvite>(`/api/networks/${id}/invites`, {
				body: { role, email: email || null }
			});
			link = new URL(resolve('/invite/[token]', { token: created.token }), page.url.origin).href;
			expires = new Date(created.expires_at).toLocaleDateString('bg-BG');
		} catch (e) {
			error = failureText(e);
		}
	}

	async function setRole(m: MemberView, r: Role) {
		error = null;
		try {
			await api(`/api/networks/${id}/members/${m.account_id}`, {
				method: 'PATCH',
				body: { role: r }
			});
			await load();
		} catch (e) {
			error = failureText(e);
		}
	}

	async function remove(m: MemberView) {
		error = null;
		try {
			await api(`/api/networks/${id}/members/${m.account_id}`, { method: 'DELETE' });
			if (m.account_id === account.me?.id) {
				await networks.reload();
				await goto(resolve('/app'));
			} else {
				await load();
			}
		} catch (e) {
			error = failureText(e);
		}
	}

	const roleText: Record<Role, string> = {
		owner: 'Собственик',
		admin: 'Администратор',
		viewer: 'Наблюдател'
	};
	const roleHelp: Record<Role, string> = {
		viewer: 'вижда мрежата, устройствата и задачите',
		admin: 'и пуска задачи, управлява устройствата и правилата',
		owner: 'и управлява членовете и може да изтрие мрежата'
	};
	const offered = $derived(
		(['viewer', 'admin', 'owner'] as Role[]).filter((r) => can(view.role, r))
	);
</script>

<svelte:head>
	<title>Членове · {view.name} · Рояк</title>
</svelte:head>

{#if error}<p class="notice bad" role="alert">{error}</p>{/if}

<section class="card">
	<h2>Членове</h2>
	<ul class="members">
		{#each members as m (m.account_id)}
			<li data-testid="member">
				<div class="who">
					<strong>{m.name}{m.account_id === account.me?.id ? ' (вие)' : ''}</strong>
					<span class="muted">{m.email}</span>
				</div>
				<div class="acts">
					{#if owner && m.account_id !== account.me?.id}
						<select
							value={m.role}
							aria-label="Роля на {m.name}"
							onchange={(e) => setRole(m, e.currentTarget.value as Role)}
						>
							{#each ['viewer', 'admin', 'owner'] as r (r)}
								<option value={r}>{roleText[r as Role]}</option>
							{/each}
						</select>
					{:else}
						<span class="badge">{roleText[m.role]}</span>
					{/if}
					{#if owner || m.account_id === account.me?.id}
						<ConfirmButton
							label={m.account_id === account.me?.id ? 'Напусни' : 'Премахни'}
							confirm="Сигурно?"
							onconfirm={() => remove(m)}
						/>
					{/if}
				</div>
			</li>
		{/each}
	</ul>
</section>

{#if admin}
	<section class="card">
		<h2>Покани някого</h2>
		<p class="muted">
			Създайте връзка и я изпратете на човека. С нея той си прави профил (или влиза в своя) и
			получава достъп до мрежата.
		</p>
		<form onsubmit={invite} class="invite">
			<label>
				Роля
				<select bind:value={role} data-testid="invite-role">
					{#each offered as r (r)}
						<option value={r}>{roleText[r]}</option>
					{/each}
				</select>
			</label>
			<label>
				Имейл (по желание)
				<input type="email" bind:value={email} placeholder="name@school.bg" />
			</label>
			<button type="submit" data-testid="create-invite">Създай покана</button>
		</form>
		<p class="muted hint">{roleText[role]} {roleHelp[role]}.</p>
		{#if link}
			<div class="link">
				<CopyField value={link} label="Връзка за покана" testid="invite-link" />
				<p class="muted hint">Валидна до {expires}, за един човек.</p>
			</div>
		{/if}
	</section>
{/if}

<style>
	section + section {
		margin-top: var(--s-4);
	}
	h2 {
		font-size: var(--text-lg);
		margin: 0 0 var(--s-3);
	}
	.members {
		list-style: none;
		margin: 0;
		padding: 0;
	}
	.members li {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: var(--s-3);
		flex-wrap: wrap;
		padding: 12px 0;
		border-bottom: 1px solid var(--border);
	}
	.members li:last-child {
		border-bottom: 0;
	}
	.who {
		display: grid;
		min-width: 0;
		overflow-wrap: anywhere;
	}
	.who .muted {
		font-size: var(--text-sm);
	}
	.acts {
		display: flex;
		gap: var(--s-2);
		align-items: center;
	}
	.acts select {
		width: auto;
	}
	.invite {
		display: grid;
		grid-template-columns: minmax(140px, 1fr) minmax(180px, 1.5fr) auto;
		gap: var(--s-3);
		align-items: end;
	}
	.hint {
		font-size: var(--text-sm);
		margin: var(--s-3) 0 0;
	}
	.link {
		margin-top: var(--s-4);
	}
	@media (max-width: 640px) {
		.invite {
			grid-template-columns: minmax(0, 1fr);
		}
	}
</style>
