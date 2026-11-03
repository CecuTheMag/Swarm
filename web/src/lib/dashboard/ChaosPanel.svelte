<script lang="ts">
	import { onMount } from 'svelte';
	import { authHeaders, loadPassword } from '#lib/password.ts';
	import { deviceLabel, type Totals } from '#lib/dashboard/state.ts';
	import type { AddCheaters } from '#lib/protocol/AddCheaters.ts';
	import type { ApiError } from '#lib/protocol/ApiError.ts';
	import type { WorkerSummary } from '#lib/protocol/WorkerSummary.ts';

	// The organiser's controls for the live demo (spec 0008): add cheaters, switch a
	// device off, cut one off. Shown only on a device that knows the submit password.

	let {
		workers,
		caughtIds,
		totals
	}: { workers: WorkerSummary[]; caughtIds: Set<string>; totals: Totals } = $props();

	let password = $state('');
	let message = $state<{ text: string; bad: boolean } | null>(null);
	let busy = $state(false);
	onMount(() => {
		password = loadPassword();
	});

	// Devices the room brought first: switching off the server's own workers proves less.
	const targets = $derived.by(() => {
		const live = workers.filter((w) => !caughtIds.has(w.id));
		const browsers = live.filter((w) => w.device.backend !== 'native');
		return browsers.length > 0 ? browsers : live;
	});

	async function call(path: string, body?: AddCheaters): Promise<boolean> {
		busy = true;
		message = null;
		try {
			const res = await fetch(`/api/chaos/${path}`, {
				method: 'POST',
				headers: { ...authHeaders(password), ...(body && { 'content-type': 'application/json' }) },
				body: body && JSON.stringify(body)
			});
			if (res.ok) return true;
			message = {
				bad: true,
				text:
					res.status === 401
						? 'Грешна парола: въведете я отново на таблото.'
						: res.status === 404
							? 'Устройството вече не е свързано.'
							: ((await res.json()) as ApiError).error
			};
		} catch (e) {
			message = { bad: true, text: e instanceof Error ? e.message : String(e) };
		} finally {
			busy = false;
		}
		return false;
	}

	async function addCheaters(collude: boolean) {
		const count = collude ? 2 : 1;
		if (await call('cheaters', { count, collude })) {
			message = {
				bad: false,
				text: collude
					? 'Двама измамници в сговор се включват: връщат един и същ грешен резултат.'
					: 'Измамник се включва: връща измислени резултати.'
			};
		}
	}

	async function disrupt(action: 'kill' | 'cut') {
		const target = targets[Math.floor(Math.random() * targets.length)];
		if (!target) return;
		if (await call(`workers/${target.id}/${action}`)) {
			const who = deviceLabel(target.device);
			message = {
				bad: false,
				text:
					action === 'kill'
						? `Изключено: ${who}. Частите му се връщат в опашката.`
						: `Без връзка: ${who}. Координаторът ще го забележи, когато замлъкне.`
			};
		}
	}
</script>

{#if password}
	<section class="chaos" data-testid="chaos-panel" aria-label="Хаос">
		<h2>Хаос</h2>
		<div class="actions">
			<button disabled={busy} onclick={() => addCheaters(false)} data-testid="chaos-cheater">
				Добави измамник
			</button>
			<button disabled={busy} onclick={() => addCheaters(true)} data-testid="chaos-colluders">
				Двама в сговор
			</button>
			<button
				class="secondary"
				disabled={busy || targets.length === 0}
				onclick={() => disrupt('kill')}
				data-testid="chaos-kill"
			>
				Изключи устройство
			</button>
			<button
				class="secondary"
				disabled={busy || targets.length === 0}
				onclick={() => disrupt('cut')}
				data-testid="chaos-cut"
			>
				Прекъсни връзката
			</button>
		</div>
		{#if message}
			<p class:bad={message.bad} class="message" role="status" data-testid="chaos-message">
				{message.text}
			</p>
		{/if}
		<p class="muted totals" data-testid="chaos-totals">
			Изключили се: {totals.left} · върнати части: {totals.unitsReturned} · блокирани: {totals.banned}
		</p>
	</section>
{/if}

<style>
	.chaos {
		border: 1px dashed var(--border);
		border-radius: var(--radius);
		padding: 10px 12px;
	}
	.chaos h2 {
		font-size: 0.875rem;
		text-transform: uppercase;
		letter-spacing: 0.08em;
		color: var(--muted);
		margin-bottom: 8px;
	}
	.actions {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 6px;
	}
	.actions button {
		padding: 8px 10px;
		font-size: 0.875rem;
	}
	.message,
	.totals {
		font-size: 0.8125rem;
		margin: 8px 0 0;
	}
</style>
