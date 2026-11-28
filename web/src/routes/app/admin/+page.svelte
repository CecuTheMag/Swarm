<script lang="ts">
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { api, failureText } from '#lib/api.ts';
	import { account } from '#lib/account.svelte.ts';
	import CopyField from '#lib/ui/CopyField.svelte';
	import type { AccountView } from '#lib/protocol/AccountView.ts';
	import type { CreatedInvite } from '#lib/protocol/CreatedInvite.ts';
	import type { ProgramSummary } from '#lib/protocol/ProgramSummary.ts';
	import type { FeatureFlag } from '#lib/protocol/FeatureFlag.ts';
	import { flags as flagStates } from '#lib/flags.svelte.ts';

	// The platform admin: who has an account, and invitations for new ones (spec 0010).
	let accounts = $state<AccountView[]>([]);
	let email = $state('');
	let link = $state<string | null>(null);
	let error = $state<string | null>(null);

	// Programs waiting for review before they may run on the public network (spec 0012).
	let pending = $state<ProgramSummary[]>([]);
	let notes = $state<Record<string, string>>({});

	// Features switched on and off without a deploy (spec 0020).
	let featureFlags = $state<FeatureFlag[]>([]);

	async function load() {
		try {
			[accounts, pending, featureFlags] = await Promise.all([
				api<AccountView[]>('/api/admin/accounts'),
				api<ProgramSummary[]>('/api/admin/programs'),
				api<FeatureFlag[]>('/api/admin/flags')
			]);
		} catch (e) {
			error = failureText(e);
		}
	}

	async function switchFlag(key: string, enabled: boolean) {
		error = null;
		try {
			const flag = await api<FeatureFlag>(`/api/admin/flags/${key}`, {
				method: 'PATCH',
				body: { enabled }
			});
			featureFlags = featureFlags.map((f) => (f.key === key ? flag : f));
			flagStates.set(key, flag.enabled);
		} catch (e) {
			error = failureText(e);
			await load();
		}
	}

	async function decide(program: string, approve: boolean) {
		error = null;
		try {
			await api(`/api/admin/programs/${program}/decision`, {
				body: { approve, note: notes[program] || undefined }
			});
			await load();
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

	<section class="card" data-testid="review-queue">
		<h2>Програми за преглед ({pending.length})</h2>
		<p class="muted">
			Одобрена програма може да се изчислява на устройството на всеки доброволец, който е позволил
			програми от общността. Сървърът вече е проверил, че модулът няма достъп до нищо извън паметта
			си; прегледът решава дали програмата прави това, което описанието ѝ казва. Одобрението важи
			само за този модул: променена програма е нова програма.
		</p>
		{#if pending.length === 0}
			<p class="muted">Няма програми, които чакат.</p>
		{/if}
		{#each pending as p (p.id)}
			<article class="pending" data-testid="pending" data-id={p.id}>
				<div>
					<strong>{p.name}</strong>
					<span class="muted"> · мрежа „{p.network_name}“ · {(p.size / 1024).toFixed(1)} KiB</span>
				</div>
				{#if p.description}<p>{p.description}</p>{/if}
				<p class="small">
					{#if p.source_url}
						Изходен код: <a href={p.source_url} rel="external noopener noreferrer">{p.source_url}</a
						>
					{:else}
						<span class="bad">Без връзка към изходния код.</span>
					{/if}
				</p>
				<p class="muted small hash">SHA-256: <code>{p.sha256}</code></p>
				<input bind:value={notes[p.id]} maxlength="2000" placeholder="Бележка към мрежата" />
				<div class="decide">
					<button data-testid="approve" onclick={() => decide(p.id, true)}>Одобри</button>
					<button class="secondary" data-testid="reject" onclick={() => decide(p.id, false)}
						>Откажи</button
					>
				</div>
			</article>
		{/each}
	</section>

	<section class="card" data-testid="feature-flags">
		<h2>Функции</h2>
		<p class="muted">
			Изключената функция изчезва от конзолата, а сървърът отказва заявките за нея. Промяната важи
			веднага, без ново внедряване.
		</p>
		{#each featureFlags as f (f.key)}
			<label class="flag">
				<input
					type="checkbox"
					role="switch"
					checked={f.enabled}
					data-testid={`flag-${f.key}`}
					onchange={(e) => switchFlag(f.key, e.currentTarget.checked)}
				/>
				<span>
					<strong>{f.description}</strong>
					<span class="muted small">
						<code>{f.key}</code> · {f.enabled ? 'включена' : 'изключена'}{f.updated_by
							? ` · ${f.updated_by}, ${new Date(f.updated_at).toLocaleString('bg-BG')}`
							: ''}
					</span>
				</span>
			</label>
		{/each}
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
	.pending {
		display: grid;
		gap: var(--s-2);
		padding: var(--s-3) 0;
		border-top: 1px solid var(--border);
	}
	.pending p {
		margin: 0;
	}
	.decide {
		display: flex;
		gap: var(--s-3);
	}
	.small {
		font-size: var(--text-sm);
	}
	.flag {
		display: flex;
		gap: var(--s-3);
		align-items: flex-start;
		padding: var(--s-3) 0;
		border-top: 1px solid var(--border);
	}
	.flag > span {
		display: grid;
		gap: var(--s-1);
	}
	.flag input {
		margin-top: 4px;
	}
	.hash {
		word-break: break-all;
	}
	@media (max-width: 560px) {
		.invite {
			flex-direction: column;
		}
	}
</style>
