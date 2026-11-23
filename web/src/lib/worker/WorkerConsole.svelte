<script lang="ts">
	import { onDestroy, onMount, untrack } from 'svelte';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { WorkerSession, type SessionStatus } from '#lib/worker/session.svelte.ts';
	import { ScreenAwake } from '#lib/worker/wake-lock.svelte.ts';
	import { DeviceConditions } from '#lib/worker/conditions.svelte.ts';
	import {
		LOW_BATTERY,
		POWER_LEVELS,
		loadPower,
		pauseReason,
		savePower,
		type PauseReason,
		type Power
	} from '#lib/worker/pacing.ts';
	import { loadCommunity, saveCommunity } from '#lib/worker/consent.ts';
	import { wsUrl } from '#lib/net.ts';
	import Icon from '#lib/ui/Icon.svelte';
	import NetworkCard from '#lib/worker/NetworkCard.svelte';
	import type { Backend } from '#lib/protocol/Backend.ts';
	import type { DeviceControl } from '#lib/protocol/DeviceControl.ts';
	import type { ProgramSummary } from '#lib/protocol/ProgramSummary.ts';

	// The page a device joins from (spec 0010): `joinCode` names the network of the link;
	// without one the device joins the public network.
	let { joinCode }: { joinCode?: string } = $props();

	// `?backend=wasm` computes on the CPU even when there is a GPU (for comparisons).
	const session = new WorkerSession(
		wsUrl('/ws/worker'),
		page.url.searchParams.get('backend') === 'wasm' ? 'wasm' : undefined,
		// Fixed for the page's life: a new link is a new page.
		untrack(() => joinCode)
	);
	const awake = new ScreenAwake();
	const conditions = new DeviceConditions();
	session.power = loadPower();
	onDestroy(() => {
		session.destroy();
		awake.destroy();
		conditions.destroy();
	});

	function choosePower(power: Power) {
		session.power = power;
		savePower(power);
	}

	// Reviewed programs of other networks (spec 0012): the contributor's choice.
	let community = $state(loadCommunity());
	session.community = untrack(() => community);
	function chooseCommunity(allowed: boolean) {
		community = allowed;
		session.community = allowed;
		saveCommunity(allowed);
	}
	let approved = $state<ProgramSummary[]>([]);
	onMount(async () => {
		const res = await fetch('/api/programs/approved').catch(() => null);
		if (res?.ok) approved = (await res.json()) as ProgramSummary[];
	});

	// `?background` keeps computing while the page is hidden: the presenting laptop's
	// worker tab behind the projector, later lab PCs. The battery still pauses.
	const background = page.url.searchParams.has('background');

	// Hidden page or low battery: pause; back to normal: resume (spec 0007).
	$effect(() => {
		session.setPause(pauseReason(conditions.hidden && !background, conditions.battery));
	});

	const backendText: Record<Backend, string> = {
		webgpu: 'Видеокарта',
		wasm: 'Процесор',
		native: 'Сървър'
	};

	// What is happening, said plainly: a headline and one line of explanation.
	const statusText: Record<SessionStatus, { title: string; detail: string }> = {
		stopped: {
			title: 'Готово за включване',
			detail: 'Натиснете бутона и устройството ще започне да помага.'
		},
		starting: { title: 'Подготовка…', detail: 'Проверяваме какво може това устройство.' },
		connecting: { title: 'Свързване…', detail: 'Само секунда.' },
		computing: {
			title: 'Помагате в момента',
			detail: 'Устройството изчислява част от обща задача. Можете да спрете по всяко време.'
		},
		waiting: {
			title: 'Включено',
			detail: 'В момента няма работа. Ще започнем, щом се появи нова задача.'
		},
		reconnecting: {
			title: 'Връзката прекъсна',
			detail: 'Свързваме се отново автоматично. Не е нужно да правите нищо.'
		},
		paused: { title: 'На пауза', detail: 'Ще продължим сами, щом е възможно.' },
		held: { title: 'Включено, не изчислява', detail: 'Мрежата не дава работа в момента.' },
		unsupported: {
			title: 'Този браузър не може да помага',
			detail: 'Нужен е по-нов браузър.'
		}
	};

	const pauseText: Record<PauseReason, string> = {
		hidden: 'Страницата е скрита. Изчисленията продължават, щом се върнете.',
		battery: `Батерията е под ${LOW_BATTERY * 100} %. Изчисленията продължават при зареждане.`
	};

	/** The owner's decision, said plainly (spec 0010). */
	function controlText(control: DeviceControl): string | null {
		switch (control.state) {
			case 'running':
				return null;
			case 'paused_by_owner':
				return 'Собственикът на мрежата спря това устройство на пауза.';
			case 'network_paused':
				return 'Мрежата е на пауза. Продължава автоматично, щом собственикът я пусне.';
			case 'outside_hours':
				return control.resumes_at
					? `Извън работното време на мрежата. Продължава автоматично ${new Date(
							control.resumes_at
						).toLocaleString('bg-BG', {
							weekday: 'long',
							hour: '2-digit',
							minute: '2-digit'
						})}.`
					: 'Извън работното време на мрежата.';
			case 'pending_approval':
				return 'Чака одобрение от администратор на мрежата. Ще започне веднага след това.';
			case 'removed':
				return 'Устройството е премахнато от мрежата.';
		}
	}

	const powerText: Record<Power, string> = {
		25: 'Леко',
		50: 'Умерено',
		75: 'Препоръчано',
		100: 'Максимум'
	};

	const active = $derived(!['stopped', 'unsupported'].includes(session.status));
	const ownerNotice = $derived(active ? controlText(session.control) : null);
	// When the queue runs dry the coordinator says to ask again in 2 s, so even a short
	// gap between jobs reads "waiting" for at least that long. The page keeps saying it
	// is computing for 3 s: a gap the next request fills never shows, an empty queue does.
	const HOLD_MS = 3000;
	let shown = $state<SessionStatus>(untrack(() => session.status));
	$effect(() => {
		const now = session.status;
		if (now === 'waiting' && untrack(() => shown) === 'computing') {
			const hold = setTimeout(() => (shown = 'waiting'), HOLD_MS);
			return () => clearTimeout(hold);
		}
		shown = now;
	});
	const busy = $derived(shown === 'computing');
	const status = $derived(statusText[shown]);
	// A phone's screen stays on while it computes; a paused one may sleep.
	$effect(() => {
		void awake.set(active && session.status !== 'paused');
	});
</script>

<main class="worker">
	<section class="console card" class:active class:busy aria-labelledby="status-title">
		<div class="ring-wrap">
			<svg class="ring" viewBox="0 0 120 120" aria-hidden="true">
				<circle class="track" cx="60" cy="60" r="54" />
			</svg>
			<!-- Its own layer: the whole SVG turns, so the compositor does it and a device
			     that is computing spends no main-thread time on it. -->
			<svg class="ring turn" viewBox="0 0 120 120" aria-hidden="true">
				<circle class="arc" cx="60" cy="60" r="54" pathLength="100" />
			</svg>
			{#if active}
				<button class="power-btn stop" onclick={() => session.stop()}>
					<Icon name="pause" size={30} />
					<span>Спри</span>
				</button>
			{:else}
				<button
					class="power-btn start"
					onclick={() => session.start()}
					disabled={session.status === 'unsupported'}
				>
					<Icon name="power" size={30} />
					<span>Включи се</span>
				</button>
			{/if}
		</div>

		<div class="state" aria-live="polite">
			<h1 id="status-title">
				<span data-testid="status" data-status={session.status}>{status.title}</span>
			</h1>
			<p class="muted">{status.detail}</p>
			{#if active && awake.held}
				<span class="badge" data-testid="screen-awake">
					<Icon name="phone" size={14} /> Екранът остава включен
				</span>
			{/if}
		</div>

		{#if active && session.pausedFor}
			<p class="notice warn" role="status" data-testid="paused" data-reason={session.pausedFor}>
				<Icon name={session.pausedFor === 'battery' ? 'battery' : 'eye'} size={18} />
				{pauseText[session.pausedFor]}
			</p>
		{/if}

		{#if ownerNotice}
			<p
				class="notice"
				class:warn={session.control.state !== 'removed'}
				class:bad={session.control.state === 'removed'}
				role="status"
				data-testid="owner-notice"
				data-control={session.control.state}
			>
				<Icon name={session.control.state === 'outside_hours' ? 'info' : 'pause'} size={18} />
				{ownerNotice}
			</p>
		{/if}

		{#if session.unfitForAi}
			<p class="notice warn" role="status" data-testid="unfit-ai">
				<Icon name="info" size={18} />
				Графичният процесор на това устройство не изчислява езиковия модел вярно, затова то не получава
				въпроси към модела. Другите задачи продължават.
			</p>
		{/if}

		{#if session.status === 'unsupported'}
			<p class="notice bad" role="alert">
				<Icon name="alert" size={18} />
				<span>
					Обновете браузъра или отворете страницата в Chrome, Edge или Safari.
					{#if session.error}<span class="muted err">{session.error}</span>{/if}
				</span>
			</p>
		{:else if session.error}
			<p class="notice bad" role="alert"><Icon name="alert" size={18} /> {session.error}</p>
		{/if}
	</section>

	<section class="card power-card" aria-labelledby="power-label">
		<div class="power-head">
			<h2 id="power-label">Мощност</h2>
			<span class="muted">{powerText[session.power as Power] ?? `${session.power} %`}</span>
		</div>
		<div class="segmented power" role="group" aria-labelledby="power-label">
			{#each POWER_LEVELS as level (level)}
				<button
					aria-pressed={session.power === level}
					data-testid="power-{level}"
					disabled={level > session.maxPower}
					onclick={() => choosePower(level)}>{level} %</button
				>
			{/each}
		</div>
		{#if session.maxPower < 100}
			<p class="muted hint" data-testid="power-limit">
				Мрежата позволява до {session.maxPower} %: устройството ползва {session.effectivePower} %.
			</p>
		{/if}
		<p class="muted hint">
			{session.power === 100
				? 'Устройството изчислява без почивка. Подходящо за компютър на зарядно.'
				: `Устройството изчислява ${session.power} % от времето и почива останалото, за да не се нагрява.`}
			{background ? 'Продължава и когато страницата е скрита.' : ''}
		</p>
	</section>

	<section class="card consent-card" aria-labelledby="consent-label">
		<label class="consent">
			<input
				type="checkbox"
				checked={community}
				disabled={active}
				data-testid="community"
				onchange={(e) => chooseCommunity(e.currentTarget.checked)}
			/>
			<span>
				<strong id="consent-label">Програми от общността</strong>
				<span class="muted">
					Изчислявай и програми на други хора, одобрени след преглед{active
						? '. Спрете, за да промените'
						: ''}. Никоя програма няма достъп до нищо извън паметта си: нито до мрежата, нито до
					файловете ви. Програмите на мрежата, в която сте се включили, се изпълняват винаги.
				</span>
			</span>
		</label>
		{#if approved.length > 0}
			<details class="approved">
				<summary>Одобрени програми ({approved.length})</summary>
				<ul>
					{#each approved as p (p.id)}
						<li>
							<strong>{p.name}</strong> · {p.network_name}{p.description
								? ` — ${p.description}`
								: ''}
							{#if p.source_url}
								<a href={p.source_url} rel="external noopener noreferrer">код</a>
							{/if}
						</li>
					{/each}
				</ul>
			</details>
		{/if}
	</section>

	<NetworkCard code={joinCode ?? 'public'} mine={session.unitsAccepted} />
	{#if !joinCode}
		<p class="muted hint other">
			Имате код от училище или организация? <a href={resolve('/join')}
				>Включете се в нейната мрежа</a
			>.
		</p>
	{/if}

	<section class="stats" aria-label="Вашият принос">
		<div class="card stat">
			<div class="value" data-testid="accepted">{session.unitsAccepted}</div>
			<div class="label">
				изчислени части{session.unitsRejected > 0 ? ` · ${session.unitsRejected} отхвърлени` : ''}
			</div>
		</div>
		<div class="card stat">
			<div class="value">{session.lastUnitMs === null ? '—' : `${session.lastUnitMs} ms`}</div>
			<div class="label">за последната част</div>
		</div>
		<div class="card stat">
			<div class="value device" data-testid="device" data-backend={session.device?.backend}>
				{session.device ? backendText[session.device.backend] : '—'}
			</div>
			<div class="label">
				{session.device
					? `${session.device.vendor} · ${session.device.architecture}`
					: 'с какво смята'}
			</div>
		</div>
	</section>

	<ul class="promises">
		<li><Icon name="lock" size={18} /> Без лични данни и без профил</li>
		<li><Icon name="eye" size={18} /> Спира, когато превключите на друго</li>
		<li><Icon name="shield" size={18} /> Само проверени изчисления</li>
		<li>
			<Icon name="info" size={18} />
			<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- resolve() builds the path; only a #section is added -->
			<a href={resolve('/') + '#safety'}>Как пазим устройството ви</a>
		</li>
	</ul>
</main>

<style>
	.worker {
		max-width: 760px;
		display: grid;
		gap: var(--s-4);
		padding-top: var(--s-6);
	}

	/* ---------- Community programs ---------- */
	.consent {
		display: flex;
		gap: var(--s-3);
		align-items: flex-start;
		cursor: pointer;
	}
	.consent input {
		width: 20px;
		height: 20px;
		margin-top: 3px;
		flex: none;
	}
	.consent > span {
		display: grid;
		gap: 4px;
		font-size: var(--text-sm);
		line-height: 1.5;
	}
	.consent strong {
		color: var(--text);
		font-size: var(--text-md);
	}
	.approved {
		margin-top: var(--s-3);
	}
	.approved summary {
		cursor: pointer;
		color: var(--muted);
	}
	.approved ul {
		margin: var(--s-2) 0 0;
		padding-left: var(--s-5);
		display: grid;
		gap: 4px;
	}

	/* ---------- The console: button, ring, status ---------- */
	.console {
		display: grid;
		justify-items: center;
		gap: var(--s-5);
		text-align: center;
		padding: clamp(var(--s-6), 6vw, var(--s-8)) var(--s-5);
		border-radius: var(--radius-xl);
	}
	.ring-wrap {
		position: relative;
		width: clamp(200px, 52vw, 248px);
		aspect-ratio: 1;
		display: grid;
		place-items: center;
	}
	.ring {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		transform: rotate(-90deg);
	}
	.track {
		fill: none;
		stroke: rgb(118 118 128 / 0.14);
		stroke-width: 4;
	}
	.arc {
		fill: none;
		stroke: var(--brand);
		stroke-width: 4;
		stroke-linecap: round;
		stroke-dasharray: 0 100;
		transition: stroke-dasharray var(--slow) var(--ease);
	}
	.turn {
		opacity: 0;
		transition: opacity var(--base) var(--ease);
	}
	.active .turn {
		opacity: 1;
		will-change: transform;
		/* One speed: changing a running animation's duration would jump the angle. Busy
		   and waiting differ by the arc's length. */
		animation: spin 1.8s linear infinite;
	}
	.active .arc {
		stroke-dasharray: 22 78;
	}
	.busy .arc {
		stroke-dasharray: 36 64;
	}
	@keyframes spin {
		from {
			transform: rotate(-90deg);
		}
		to {
			transform: rotate(270deg);
		}
	}

	.power-btn {
		position: relative;
		width: 76%;
		height: 76%;
		border-radius: 50%;
		flex-direction: column;
		gap: 6px;
		font-size: var(--text-lg);
		font-weight: 700;
		letter-spacing: -0.02em;
		white-space: normal;
		transition:
			transform var(--base) var(--ease),
			box-shadow var(--base) var(--ease),
			background-color var(--base) var(--ease);
	}
	.power-btn.start {
		box-shadow:
			inset 0 2px 0 rgb(255 255 255 / 0.45),
			0 20px 50px -16px rgb(224 142 11 / 0.75);
	}
	/* A still halo: nothing on this page moves until the device is working. */
	.power-btn.start::before {
		content: '';
		position: absolute;
		inset: -10px;
		border-radius: 50%;
		background: radial-gradient(closest-side, rgb(245 165 36 / 0.28), transparent);
		z-index: -1;
		pointer-events: none;
	}
	@media (hover: hover) {
		.power-btn.start:hover {
			transform: scale(1.02);
		}
		.power-btn.stop:hover {
			background: var(--bg-elevated);
			color: var(--bad);
		}
	}
	.power-btn:active,
	.power-btn.start:active {
		transform: scale(0.97);
		transition-duration: var(--press);
	}
	.power-btn.stop {
		background: var(--glass-strong);
		color: var(--text);
		border: 1px solid var(--border-strong);
		box-shadow: var(--shadow);
	}
	.power-btn:disabled::before {
		opacity: 0;
	}

	.state {
		display: grid;
		justify-items: center;
		gap: var(--s-2);
		max-width: 46ch;
	}
	.state h1 {
		font-size: clamp(1.6rem, 1.2rem + 1.6vw, 2.25rem);
		letter-spacing: -0.03em;
		margin: 0;
	}
	/* Room for the longest status line, so the card does not grow when the device starts. */
	.state p {
		margin: 0;
		font-size: var(--text-md);
		min-height: 2lh;
	}
	@media (max-width: 480px) {
		.state p {
			min-height: 3lh;
		}
	}
	.notice {
		text-align: left;
		width: 100%;
		margin: 0;
	}
	.err {
		display: block;
		font-size: var(--text-xs);
		margin-top: 4px;
		overflow-wrap: anywhere;
	}

	/* ---------- Power ---------- */
	.power-card {
		display: grid;
		gap: var(--s-3);
	}
	.power-head {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--s-3);
	}
	.power-head h2 {
		font-size: var(--text-lg);
		margin: 0;
	}
	.power {
		width: 100%;
	}
	.hint {
		margin: 0;
		font-size: var(--text-sm);
	}
	.other {
		text-align: center;
	}

	/* ---------- Stats ---------- */
	.stats {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: var(--s-4);
	}
	.stats .card {
		padding: var(--s-4) var(--s-5);
		min-width: 0;
	}
	.device {
		overflow-wrap: anywhere;
	}
	.stats .label {
		overflow-wrap: anywhere;
	}

	.promises {
		list-style: none;
		margin: var(--s-3) 0 0;
		padding: 0;
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: var(--s-3) var(--s-5);
		color: var(--muted);
		font-size: var(--text-sm);
	}
	.promises li {
		display: flex;
		align-items: center;
		gap: var(--s-2);
	}
	.promises :global(svg) {
		flex: none;
		color: var(--brand-ink);
	}

	@media (max-width: 600px) {
		.stats {
			grid-template-columns: 1fr 1fr;
		}
		.stats .card:last-child {
			grid-column: 1 / -1;
		}
		.promises {
			grid-template-columns: 1fr;
		}
	}
</style>
