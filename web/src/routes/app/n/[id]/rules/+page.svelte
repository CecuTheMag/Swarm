<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { api, failureText } from '#lib/api.ts';
	import { can, networks, useNetwork } from '#lib/console/networks.svelte.ts';
	import ConfirmButton from '#lib/ui/ConfirmButton.svelte';
	import Icon from '#lib/ui/Icon.svelte';
	import type { HoursWindow } from '#lib/protocol/HoursWindow.ts';
	import type { JoinMode } from '#lib/protocol/JoinMode.ts';
	import type { NetworkView } from '#lib/protocol/NetworkView.ts';
	import type { UpdateNetwork } from '#lib/protocol/UpdateNetwork.ts';

	const network = useNetwork();
	const id = $derived(page.params.id!);
	const view = $derived(network.view!);
	const admin = $derived(can(view.role, 'admin'));
	const owner = $derived(can(view.role, 'owner'));

	// A draft of the rules, saved together.
	let name = $state('');
	let joinMode = $state<JoinMode>('open');
	let maxPower = $state(100);
	let lendIdle = $state(false);
	let useHours = $state(false);
	let timezone = $state(Intl.DateTimeFormat().resolvedOptions().timeZone || 'Europe/Sofia');
	let windows = $state<HoursWindow[]>([]);
	let loadedFor = '';
	$effect(() => {
		if (view.id === loadedFor) return;
		loadedFor = view.id;
		name = view.name;
		joinMode = view.join_mode;
		maxPower = view.max_power;
		lendIdle = view.lend_idle;
		useHours = view.hours !== null;
		if (view.hours) {
			timezone = view.hours.timezone;
			windows = view.hours.windows.map((w) => ({ ...w, days: [...w.days] }));
		}
	});

	const DAYS = ['пн', 'вт', 'ср', 'чт', 'пт', 'сб', 'нд'];
	/** Lab PCs after school: weekday evenings into the next morning, all weekend. */
	function afterSchool() {
		windows = [
			{ days: [1, 2, 3, 4, 5], from: '15:00', to: '08:00' },
			{ days: [6, 7], from: '00:00', to: '00:00' }
		];
	}
	function addWindow() {
		windows = [...windows, { days: [1, 2, 3, 4, 5], from: '08:00', to: '17:00' }];
	}
	function toggleDay(w: HoursWindow, d: number) {
		w.days = w.days.includes(d) ? w.days.filter((x) => x !== d) : [...w.days, d].sort();
	}

	let error = $state<string | null>(null);
	let saved = $state(false);
	let busy = $state(false);

	async function save(body: UpdateNetwork) {
		busy = true;
		error = null;
		saved = false;
		try {
			network.set(await api<NetworkView>(`/api/networks/${id}`, { method: 'PATCH', body }));
			saved = true;
			setTimeout(() => (saved = false), 2500);
		} catch (e) {
			error = failureText(e);
		} finally {
			busy = false;
		}
	}

	function submit(event: SubmitEvent) {
		event.preventDefault();
		if (useHours && windows.some((w) => w.days.length === 0)) {
			error = 'Всеки интервал трябва да има поне един ден.';
			return;
		}
		void save({
			name,
			join_mode: joinMode,
			max_power: maxPower,
			lend_idle: lendIdle,
			hours: useHours ? { timezone, windows } : null
		});
	}

	async function deleteNetwork() {
		try {
			await api(`/api/networks/${id}`, { method: 'DELETE' });
			await networks.reload();
			await goto(resolve('/app'));
		} catch (e) {
			error = failureText(e);
		}
	}
</script>

<svelte:head>
	<title>Правила · {view.name} · Рояк</title>
</svelte:head>

<form class="rules" onsubmit={submit}>
	<fieldset disabled={!admin}>
		<section class="card">
			<h2>Мрежата</h2>
			<label>Име <input bind:value={name} maxlength="80" required /></label>
		</section>

		<section class="card">
			<h2>Кой може да се включи</h2>
			<div class="segmented" role="group" aria-label="Включване">
				<button
					type="button"
					aria-pressed={joinMode === 'open'}
					onclick={() => (joinMode = 'open')}
					data-testid="join-open"
				>
					Всеки с връзката
				</button>
				<button
					type="button"
					aria-pressed={joinMode === 'approval'}
					onclick={() => (joinMode = 'approval')}
					data-testid="join-approval"
				>
					След одобрение
				</button>
			</div>
			<p class="muted hint">
				{joinMode === 'open'
					? 'Всяко устройство с връзката или QR кода започва веднага.'
					: 'Новите устройства чакат администратор да ги одобри в „Устройства“.'}
			</p>
		</section>

		<section class="card">
			<h2>Максимална мощност</h2>
			<div class="segmented" role="group" aria-label="Максимална мощност">
				{#each [25, 50, 75, 100] as p (p)}
					<button type="button" aria-pressed={maxPower === p} onclick={() => (maxPower = p)}
						>{p} %</button
					>
				{/each}
			</div>
			<p class="muted hint">
				Устройствата никога не изчисляват повече от това, каквото и да изберат собствениците им.
			</p>
		</section>

		<section class="card">
			<div class="row">
				<h2>Работно време</h2>
				<label class="switch">
					<input type="checkbox" bind:checked={useHours} data-testid="use-hours" />
					Само в определени часове
				</label>
			</div>
			{#if useHours}
				<p class="muted hint">Извън тези часове устройствата остават включени, но не изчисляват.</p>
				<div class="hours">
					{#each windows as w, i (i)}
						<div class="window">
							<div class="days" role="group" aria-label="Дни">
								{#each DAYS as d, j (d)}
									<button
										type="button"
										class="day"
										aria-pressed={w.days.includes(j + 1)}
										onclick={() => toggleDay(w, j + 1)}>{d}</button
									>
								{/each}
							</div>
							<label class="time">от <input type="time" bind:value={w.from} required /></label>
							<label class="time">до <input type="time" bind:value={w.to} required /></label>
							<button
								type="button"
								class="ghost sm"
								onclick={() => (windows = windows.filter((_, k) => k !== i))}
								aria-label="Премахни интервала"
							>
								<Icon name="close" size={16} />
							</button>
						</div>
					{/each}
				</div>
				<div class="row start">
					<button type="button" class="secondary sm" onclick={addWindow}>Добави интервал</button>
					<button
						type="button"
						class="secondary sm"
						onclick={afterSchool}
						data-testid="after-school">След училище</button
					>
				</div>
				<p class="muted hint">
					Интервал, чийто край е преди началото, продължава след полунощ (15:00–08:00). Часова зона:
					<code>{timezone}</code>.
				</p>
			{/if}
		</section>

		{#if !view.public}
			<section class="card">
				<div class="row">
					<h2>Помощ за общата мрежа</h2>
					<label class="switch">
						<input type="checkbox" bind:checked={lendIdle} />
						Включено
					</label>
				</div>
				<p class="muted hint">
					Когато мрежата няма своя задача, устройствата ѝ помагат на доброволната обща мрежа.
				</p>
			</section>
		{/if}

		{#if admin}
			<div class="save">
				<button type="submit" class="lg" disabled={busy} data-testid="save-rules">Запази</button>
				{#if saved}<span class="badge ok" role="status"
						><Icon name="check" size={14} /> Запазено</span
					>{/if}
			</div>
		{/if}
		{#if error}<p class="notice bad" role="alert">{error}</p>{/if}
	</fieldset>
</form>

{#if admin && !view.public}
	<section class="card danger-zone">
		<h2>Връзка за включване</h2>
		<p class="muted">
			Нова връзка спира старата: устройствата, които още не са се включили с нея, вече няма да
			могат.
		</p>
		<ConfirmButton
			label="Създай нова връзка"
			confirm="Старата спира. Продължи?"
			onconfirm={() => save({ new_join_code: true })}
			size="md"
		/>
		{#if owner}
			<h2 class="spaced">Изтриване</h2>
			<p class="muted">Изтрива мрежата, устройствата и задачите ѝ. Не може да се върне.</p>
			<ConfirmButton
				label="Изтрий мрежата"
				confirm="Изтрий окончателно"
				onconfirm={deleteNetwork}
				size="md"
				testid="delete-network"
			/>
		{/if}
	</section>
{/if}

<style>
	fieldset {
		border: 0;
		margin: 0;
		padding: 0;
		min-width: 0;
		display: grid;
		gap: var(--s-4);
	}
	h2 {
		font-size: var(--text-lg);
		margin: 0 0 var(--s-3);
	}
	.spaced {
		margin-top: var(--s-5);
	}
	.hint {
		font-size: var(--text-sm);
		margin: var(--s-3) 0 0;
	}
	.row {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: var(--s-3);
		flex-wrap: wrap;
	}
	.row h2 {
		margin: 0;
	}
	.row.start {
		justify-content: flex-start;
		margin-top: var(--s-3);
	}
	.switch {
		display: inline-flex;
		align-items: center;
		gap: 8px;
		font-size: var(--text-sm);
		color: var(--text);
	}
	.hours {
		display: grid;
		gap: var(--s-3);
		margin-top: var(--s-3);
	}
	.window {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--s-3);
		padding: var(--s-3);
		border-radius: var(--radius);
		background: rgb(118 118 128 / 0.08);
	}
	.days {
		display: flex;
		gap: 4px;
		flex-wrap: wrap;
	}
	.day {
		min-height: 36px;
		min-width: 40px;
		padding: 4px 8px;
		background: transparent;
		color: var(--text);
		border: 1px solid var(--border-strong);
		box-shadow: none;
		font-weight: 500;
	}
	.day[aria-pressed='true'] {
		background: var(--text);
		color: var(--bg);
		border-color: var(--text);
	}
	.time {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		font-size: var(--text-sm);
	}
	.time input {
		width: auto;
	}
	.save {
		display: flex;
		align-items: center;
		gap: var(--s-3);
	}
	.danger-zone {
		margin-top: var(--s-4);
	}
	.danger-zone p {
		font-size: var(--text-sm);
	}
</style>
