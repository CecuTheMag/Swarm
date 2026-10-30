<script lang="ts">
	import { onDestroy } from 'svelte';
	import { page } from '$app/state';
	import { WorkerSession, type SessionStatus } from '#lib/worker/session.svelte.ts';
	import { ScreenAwake } from '#lib/worker/wake-lock.svelte.ts';
	import { wsUrl } from '#lib/net.ts';
	import type { Backend } from '#lib/protocol/Backend.ts';

	// `?backend=wasm` computes on the CPU even when there is a GPU (for comparisons).
	const session = new WorkerSession(
		wsUrl('/ws/worker'),
		page.url.searchParams.get('backend') === 'wasm' ? 'wasm' : undefined
	);
	const awake = new ScreenAwake();
	onDestroy(() => {
		session.destroy();
		awake.destroy();
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
		unsupported: 'Не се поддържа'
	};

	const active = $derived(!['stopped', 'unsupported'].includes(session.status));
	// A phone's screen stays on while it takes part.
	$effect(() => {
		void awake.set(active);
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
	.device {
		font-size: 1rem;
		word-break: break-word;
	}
</style>
