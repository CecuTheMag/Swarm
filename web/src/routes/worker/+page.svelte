<script lang="ts">
	import { onDestroy } from 'svelte';
	import { page } from '$app/state';
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
	import { wsUrl } from '#lib/net.ts';
	import type { Backend } from '#lib/protocol/Backend.ts';

	// `?backend=wasm` computes on the CPU even when there is a GPU (for comparisons).
	const session = new WorkerSession(
		wsUrl('/ws/worker'),
		page.url.searchParams.get('backend') === 'wasm' ? 'wasm' : undefined
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

	// `?background` keeps computing while the page is hidden: the presenting laptop's
	// worker tab behind the projector, later lab PCs. The battery still pauses.
	const background = page.url.searchParams.has('background');

	// Hidden page or low battery: pause; back to normal: resume (spec 0007).
	$effect(() => {
		session.setPause(pauseReason(conditions.hidden && !background, conditions.battery));
	});

	const backendText: Record<Backend, string> = {
		webgpu: 'видеокарта',
		wasm: 'процесор',
		native: 'native'
	};

	const statusText: Record<SessionStatus, string> = {
		stopped: 'Спрян',
		starting: 'Подготовка…',
		connecting: 'Свързване…',
		computing: 'Изчислява',
		waiting: 'Изчаква задачи',
		reconnecting: 'Повторно свързване…',
		paused: 'На пауза',
		unsupported: 'Не се поддържа'
	};

	const pauseText: Record<PauseReason, string> = {
		hidden: 'Страницата е скрита. Изчисленията продължават, щом се върнете.',
		battery: `Батерията е под ${LOW_BATTERY * 100} %. Изчисленията продължават при зареждане.`
	};

	const active = $derived(!['stopped', 'unsupported'].includes(session.status));
	// A phone's screen stays on while it computes; a paused one may sleep.
	$effect(() => {
		void awake.set(active && session.status !== 'paused');
	});
</script>

<main>
	<h1>Дари изчислителна мощ</h1>
	<p class="muted">
		Тази страница изпълнява само проверени изчислителни шаблони на Рояк чрез видеокартата на
		устройството, а ако няма подходяща видеокарта — чрез процесора. Не се събират лични данни.
		Можете да спрете по всяко време.
	</p>

	<div class="card controls">
		{#if active}
			<button class="secondary" onclick={() => session.stop()}>Спри</button>
		{:else}
			<button onclick={() => session.start()} disabled={session.status === 'unsupported'}>
				Включи се
			</button>
		{/if}
		<span data-testid="status" data-status={session.status}>{statusText[session.status]}</span>
		{#if active && awake.held}
			<span class="muted" data-testid="screen-awake">Екранът остава включен</span>
		{/if}
	</div>

	<div class="power" role="group" aria-labelledby="power-label">
		<span id="power-label">Мощност</span>
		{#each POWER_LEVELS as level (level)}
			<button
				class:secondary={session.power !== level}
				aria-pressed={session.power === level}
				data-testid="power-{level}"
				onclick={() => choosePower(level)}>{level} %</button
			>
		{/each}
	</div>
	<p class="muted hint">
		{session.power === 100
			? 'Устройството изчислява без почивка.'
			: `Устройството изчислява ${session.power} % от времето и почива останалото.`}
		{background ? 'Продължава и когато страницата е скрита.' : ''}
	</p>

	{#if active && session.pausedFor}
		<p class="paused" role="status" data-testid="paused" data-reason={session.pausedFor}>
			{pauseText[session.pausedFor]}
		</p>
	{/if}

	{#if session.status === 'unsupported'}
		<p class="bad" role="alert">
			Този браузър не може да изчислява за Рояк. Опитайте с актуален Chrome или Edge.
		</p>
		<p class="muted">{session.error}</p>
	{:else if session.error}
		<p class="bad" role="alert">{session.error}</p>
	{/if}

	<div class="grid stats">
		<div class="card stat">
			<div class="value" data-testid="accepted">{session.unitsAccepted}</div>
			<div class="label">Приети единици</div>
		</div>
		<div class="card stat">
			<div class="value">{session.unitsRejected}</div>
			<div class="label">Отхвърлени</div>
		</div>
		<div class="card stat">
			<div class="value">{session.lastUnitMs === null ? '—' : `${session.lastUnitMs} ms`}</div>
			<div class="label">Последна единица</div>
		</div>
		<div class="card stat">
			<div class="value device" data-testid="device" data-backend={session.device?.backend}>
				{session.device
					? `${backendText[session.device.backend]}: ${session.device.vendor} · ${session.device.architecture}`
					: '—'}
			</div>
			<div class="label">Устройство</div>
		</div>
	</div>
</main>

<style>
	.controls {
		display: flex;
		align-items: center;
		gap: 16px;
		margin: 20px 0;
	}
	.stats {
		margin-top: 16px;
	}
	.power {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 8px;
	}
	.power span {
		margin-right: 8px;
	}
	.power button {
		min-width: 64px;
	}
	.hint {
		margin: 8px 0 0;
	}
	.paused {
		margin: 16px 0 0;
		padding: 12px 16px;
		border-radius: 8px;
		border: 1px solid currentColor;
	}
	.device {
		font-size: 1rem;
		word-break: break-word;
	}
</style>
