<script lang="ts">
	import { page } from '$app/state';
	import { api, failureText } from '#lib/api.ts';
	import { useNetwork } from '#lib/console/networks.svelte.ts';
	import ConfirmButton from '#lib/ui/ConfirmButton.svelte';
	import CopyField from '#lib/ui/CopyField.svelte';
	import type { ApiKeyView } from '#lib/protocol/ApiKeyView.ts';
	import type { CreatedApiKey } from '#lib/protocol/CreatedApiKey.ts';

	const network = useNetwork();
	const id = $derived(page.params.id!);

	let keys = $state<ApiKeyView[]>([]);
	let label = $state('');
	let created = $state<string | null>(null);
	let error = $state<string | null>(null);

	async function load() {
		try {
			keys = await api<ApiKeyView[]>(`/api/networks/${id}/keys`);
		} catch (e) {
			error = failureText(e);
		}
	}
	$effect(() => {
		void load();
	});

	async function create(event: SubmitEvent) {
		event.preventDefault();
		error = null;
		try {
			created = (await api<CreatedApiKey>(`/api/networks/${id}/keys`, { body: { label } })).key;
			label = '';
			await load();
		} catch (e) {
			error = failureText(e);
		}
	}

	async function revoke(key: string) {
		try {
			await api(`/api/networks/${id}/keys/${key}`, { method: 'DELETE' });
			await load();
		} catch (e) {
			error = failureText(e);
		}
	}

	const origin = $derived(page.url.origin);
	const example = $derived(`curl -X POST ${origin}/api/networks/${id}/jobs \\
  -H "Authorization: Bearer $SWARM_KEY" \\
  -H "content-type: application/json" \\
  -d '{"template":"pi","params":{"units":64,"samples_per_unit":16777216,"seed":1}}'`);
</script>

<svelte:head>
	<title>API · {network.view?.name} · Рояк</title>
</svelte:head>

{#if error}<p class="notice bad" role="alert">{error}</p>{/if}

<section class="card">
	<h2>Ключове за API</h2>
	<p class="muted">
		С ключ програма пуска задачи в тази мрежа и чете резултатите им, без да влиза с профил. Ключът
		действа като администратор на мрежата, но не може да кани хора.
	</p>
	<form class="create" onsubmit={create}>
		<input
			bind:value={label}
			placeholder="За какво е, напр. „Скрипт на лабораторията“"
			maxlength="80"
			required
			aria-label="Етикет на ключа"
		/>
		<button type="submit">Създай ключ</button>
	</form>
	{#if created}
		<div class="notice warn new-key" role="status">
			<div>
				<strong>Копирайте ключа сега.</strong> Показва се само веднъж.
				<CopyField value={created} label="Нов ключ за API" testid="api-key" />
			</div>
		</div>
	{/if}
	{#if keys.length > 0}
		<ul class="keys">
			{#each keys as k (k.id)}
				<li>
					<div>
						<strong>{k.label}</strong>
						<span class="muted">
							създаден {new Date(k.created_at).toLocaleDateString('bg-BG')} ·
							{k.last_used_at
								? `ползван ${new Date(k.last_used_at).toLocaleString('bg-BG')}`
								: 'неползван'}
						</span>
					</div>
					<ConfirmButton label="Отмени" confirm="Отмени ключа?" onconfirm={() => revoke(k.id)} />
				</li>
			{/each}
		</ul>
	{/if}
</section>

<section class="card">
	<h2>Пример</h2>
	<pre><code>{example}</code></pre>
</section>

<style>
	section + section {
		margin-top: var(--s-4);
	}
	h2 {
		font-size: var(--text-lg);
		margin: 0 0 var(--s-3);
	}
	.create {
		display: flex;
		gap: var(--s-3);
		margin: var(--s-3) 0;
	}
	.new-key > div {
		display: grid;
		gap: var(--s-2);
		width: 100%;
	}
	.keys {
		list-style: none;
		margin: var(--s-3) 0 0;
		padding: 0;
	}
	.keys li {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: var(--s-3);
		padding: 12px 0;
		border-bottom: 1px solid var(--border);
	}
	.keys li:last-child {
		border-bottom: 0;
	}
	.keys li div {
		display: grid;
		min-width: 0;
	}
	.keys .muted {
		font-size: var(--text-sm);
	}
	pre {
		margin: 0;
		padding: var(--s-4);
		border-radius: var(--radius);
		background: rgb(118 118 128 / 0.1);
		overflow-x: auto;
		font-size: 13px;
	}
	@media (max-width: 560px) {
		.create {
			flex-direction: column;
		}
	}
</style>
