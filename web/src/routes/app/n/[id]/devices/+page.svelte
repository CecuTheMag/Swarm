<script lang="ts">
	import { page } from '$app/state';
	import { api, failureText } from '#lib/api.ts';
	import { can, useNetwork } from '#lib/console/networks.svelte.ts';
	import ConfirmButton from '#lib/ui/ConfirmButton.svelte';
	import Icon from '#lib/ui/Icon.svelte';
	import type { DeviceView } from '#lib/protocol/DeviceView.ts';

	const network = useNetwork();
	const id = $derived(page.params.id!);
	const admin = $derived(network.view ? can(network.view.role, 'admin') : false);

	let devices = $state<DeviceView[] | null>(null);
	let error = $state<string | null>(null);
	let selected = $state<string[]>([]);
	let editing = $state<string | null>(null);
	let draft = $state('');

	async function load() {
		try {
			devices = await api<DeviceView[]>(`/api/networks/${id}/devices`);
		} catch (e) {
			error = failureText(e);
		}
	}
	// Online state changes on its own; refresh every few seconds while the page is open.
	$effect(() => {
		void load();
		const timer = setInterval(load, 5000);
		return () => clearInterval(timer);
	});

	async function update(deviceId: string, body: object) {
		error = null;
		try {
			await api(`/api/networks/${id}/devices/${deviceId}`, { method: 'PATCH', body });
			await load();
		} catch (e) {
			error = failureText(e);
		}
	}

	async function remove(deviceId: string) {
		error = null;
		try {
			await api(`/api/networks/${id}/devices/${deviceId}`, { method: 'DELETE' });
			selected = selected.filter((s) => s !== deviceId);
			await load();
		} catch (e) {
			error = failureText(e);
		}
	}

	async function bulk(body: object) {
		for (const d of selected) await update(d, body);
		selected = [];
	}

	function rename(d: DeviceView) {
		editing = d.id;
		draft = d.name ?? '';
	}

	async function saveName(d: DeviceView) {
		editing = null;
		if ((d.name ?? '') !== draft.trim()) await update(d.id, { name: draft.trim() });
	}

	const kind: Record<string, string> = { webgpu: 'Видеокарта', wasm: 'Процесор', native: 'Сървър' };
	const when = (ms: number) => {
		const s = Math.max(0, (Date.now() - ms) / 1000);
		if (s < 60) return 'току-що';
		if (s < 3600) return `преди ${Math.floor(s / 60)} мин`;
		if (s < 86400) return `преди ${Math.floor(s / 3600)} ч`;
		return new Date(ms).toLocaleDateString('bg-BG');
	};
	const pending = $derived(devices?.filter((d) => d.status === 'pending') ?? []);
	const active = $derived(devices?.filter((d) => d.status === 'active') ?? []);
	const allSelected = $derived(active.length > 0 && selected.length === active.length);
</script>

<svelte:head>
	<title>Устройства · {network.view?.name} · Рояк</title>
</svelte:head>

{#if error}<p class="notice bad" role="alert">{error}</p>{/if}

{#if pending.length > 0}
	<section class="card pending">
		<h2>Чакат одобрение</h2>
		<ul class="list">
			{#each pending as d (d.id)}
				<li data-testid="pending-device">
					<span class="who"><Icon name="phone" size={18} /> {kind[d.backend]} · {d.vendor}</span>
					{#if admin}
						<div class="acts">
							<button
								class="sm"
								onclick={() => update(d.id, { approve: true })}
								data-testid="approve"
							>
								Одобри
							</button>
							<ConfirmButton label="Откажи" confirm="Сигурно?" onconfirm={() => remove(d.id)} />
						</div>
					{/if}
				</li>
			{/each}
		</ul>
	</section>
{/if}

<section class="card">
	<div class="head">
		<h2>Устройства</h2>
		{#if admin && selected.length > 0}
			<div class="acts" data-testid="bulk">
				<span class="muted">{selected.length} избрани</span>
				<button class="secondary sm" onclick={() => bulk({ paused: true })}>Пауза</button>
				<button class="secondary sm" onclick={() => bulk({ paused: false })}>Пусни</button>
			</div>
		{/if}
	</div>

	{#if devices === null}
		<p class="muted">Зареждане…</p>
	{:else if active.length === 0}
		<p class="muted">Още няма устройства. Добавете ги от „Обзор“ с връзката или QR кода.</p>
	{:else}
		<div class="table-wrap">
			<table>
				<thead>
					<tr>
						{#if admin}
							<th class="check">
								<input
									type="checkbox"
									aria-label="Избери всички"
									checked={allSelected}
									onchange={(e) =>
										(selected = e.currentTarget.checked ? active.map((d) => d.id) : [])}
								/>
							</th>
						{/if}
						<th>Устройство</th>
						<th>Състояние</th>
						<th>Части</th>
						<th>Последно</th>
						{#if admin}<th></th>{/if}
					</tr>
				</thead>
				<tbody>
					{#each active as d (d.id)}
						<tr
							data-testid="device-row"
							data-device-id={d.id}
							data-paused={d.paused}
							data-online={d.online}
						>
							{#if admin}
								<td class="check">
									<input
										type="checkbox"
										aria-label="Избери {d.name ?? kind[d.backend]}"
										checked={selected.includes(d.id)}
										onchange={(e) =>
											(selected = e.currentTarget.checked
												? [...selected, d.id]
												: selected.filter((s) => s !== d.id))}
									/>
								</td>
							{/if}
							<td>
								{#if editing === d.id}
									<form
										onsubmit={(e) => {
											e.preventDefault();
											void saveName(d);
										}}
									>
										<input
											class="name-input"
											bind:value={draft}
											maxlength="80"
											aria-label="Име на устройството"
											onblur={() => saveName(d)}
										/>
									</form>
								{:else}
									<div class="dev">
										<strong>{d.name ?? kind[d.backend]}</strong>
										<span class="muted">{d.vendor} · {d.architecture}</span>
									</div>
								{/if}
							</td>
							<td>
								{#if d.paused}
									<span class="badge">на пауза</span>
								{:else if d.online}
									<span class="badge ok"><span class="dot"></span> на линия</span>
								{:else}
									<span class="badge">изключено</span>
								{/if}
							</td>
							<td class="num">{d.units_done}</td>
							<td class="muted">{d.online ? 'сега' : when(d.last_seen_at)}</td>
							{#if admin}
								<td class="row-acts">
									<button class="ghost sm" onclick={() => rename(d)} aria-label="Преименувай">
										Име
									</button>
									<button
										class="secondary sm"
										onclick={() => update(d.id, { paused: !d.paused })}
										data-testid="toggle-device"
									>
										{d.paused ? 'Пусни' : 'Пауза'}
									</button>
									<ConfirmButton
										label="Премахни"
										confirm="Премахни?"
										onconfirm={() => remove(d.id)}
										testid="remove-device"
									/>
								</td>
							{/if}
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{/if}
</section>

<style>
	section + section {
		margin-top: var(--s-4);
	}
	h2 {
		font-size: var(--text-lg);
		margin: 0;
	}
	.head {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: var(--s-3);
		flex-wrap: wrap;
		margin-bottom: var(--s-3);
	}
	.acts,
	.row-acts {
		display: flex;
		gap: var(--s-2);
		align-items: center;
		flex-wrap: wrap;
	}
	.row-acts {
		justify-content: flex-end;
	}
	.list {
		list-style: none;
		margin: var(--s-3) 0 0;
		padding: 0;
		display: grid;
		gap: var(--s-2);
	}
	.list li {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: var(--s-3);
		flex-wrap: wrap;
		padding: var(--s-3);
		border-radius: var(--radius);
		background: var(--brand-soft);
	}
	.who {
		display: inline-flex;
		gap: 8px;
		align-items: center;
		font-weight: 600;
	}
	.check {
		width: 36px;
	}
	.dev {
		display: grid;
	}
	.dev .muted {
		font-size: var(--text-xs);
	}
	.num {
		font-variant-numeric: tabular-nums;
	}
	.name-input {
		min-height: 36px;
		padding: 6px 10px;
	}
	td .dot {
		width: 6px;
		height: 6px;
	}

	/* Phones: each device is a list row, not a table squeezed sideways. */
	@media (max-width: 760px) {
		table,
		tbody {
			display: block;
		}
		thead {
			display: none;
		}
		tr {
			display: flex;
			flex-wrap: wrap;
			align-items: center;
			gap: 6px var(--s-3);
			padding: var(--s-3) 0;
			border-bottom: 1px solid var(--border);
		}
		tbody tr:last-child {
			border-bottom: 0;
		}
		td {
			padding: 0;
			border: 0;
		}
		td.check {
			width: auto;
		}
		/* Line one: name and state; line two: parts and when. */
		td:has(.dev),
		td:has(form) {
			flex: 1 1 50%;
			min-width: 0;
		}
		td.num,
		td.num + td {
			font-size: var(--text-xs);
			color: var(--muted);
		}
		tr:has(td.check) td.num {
			margin-left: 32px;
		}
		td.num::after {
			content: ' части';
		}
		td.num + td::before {
			content: '· ';
		}
		/* A row's actions appear when it is selected, like a list in edit mode; without
		   selection the list stays a list. */
		td.row-acts {
			display: none;
			flex-basis: 100%;
			justify-content: flex-start;
		}
		tr:has(td.check input:checked) td.row-acts {
			display: flex;
		}
		tr:has(td.check) td.row-acts {
			padding-left: 32px;
		}
	}
</style>
