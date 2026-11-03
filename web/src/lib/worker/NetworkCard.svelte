<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { wsUrl } from '#lib/net.ts';
	import Icon from '#lib/ui/Icon.svelte';
	import type { NetworkSummary } from '#lib/protocol/NetworkSummary.ts';

	// The network this device joined, live (spec 0010): who else is in it, how fast it
	// computes, what it works on, and this device's share. Nothing about other devices'
	// owners or the jobs' data is sent.

	let { code, mine }: { code: string; mine: number } = $props();

	let summary = $state<NetworkSummary | null>(null);
	let connected = $state(false);
	let ws: WebSocket | null = null;
	let retry: ReturnType<typeof setTimeout> | undefined;
	let closed = false;

	function connect() {
		const socket = new WebSocket(wsUrl(`/ws/join/${encodeURIComponent(code)}/summary`));
		ws = socket;
		socket.onopen = () => (connected = true);
		socket.onmessage = (e) => (summary = JSON.parse(e.data) as NetworkSummary);
		socket.onclose = () => {
			connected = false;
			if (!closed) retry = setTimeout(connect, 3000);
		};
	}
	onMount(connect);
	onDestroy(() => {
		closed = true;
		clearTimeout(retry);
		ws?.close();
	});

	const devices = $derived(
		summary ? summary.devices_gpu + summary.devices_cpu + summary.devices_server : 0
	);
	const progress = $derived(
		summary?.job ? summary.job.units_done / Math.max(1, summary.job.units_total) : 0
	);
	const jobName: Record<string, string> = { pt: 'Изображение', pi: 'Числото π' };
	const plural = (n: number, one: string, many: string) => (n === 1 ? one : many);
</script>

<section class="card network" aria-label="Мрежата" data-testid="network-card">
	<header>
		<div>
			<span class="eyebrow">Мрежата</span>
			<h2 data-testid="network-name">{summary?.name ?? '…'}</h2>
		</div>
		<span class="badge" class:live={connected}>
			<span class="dot" class:pulse={connected}></span>
			{connected ? 'на живо' : 'свързване…'}
		</span>
	</header>

	<div class="figures">
		<div class="figure">
			<span class="num" data-testid="network-devices">{devices}</span>
			<span class="muted">{plural(devices, 'устройство', 'устройства')} сега</span>
			{#if summary && devices > 0}
				<span class="kinds muted">
					{#if summary.devices_gpu}<span><i class="sw gpu"></i>{summary.devices_gpu}</span>{/if}
					{#if summary.devices_cpu}<span><i class="sw cpu"></i>{summary.devices_cpu}</span>{/if}
					{#if summary.devices_server}<span><i class="sw srv"></i>{summary.devices_server}</span
						>{/if}
				</span>
			{/if}
		</div>
		<div class="figure">
			<span class="num">
				{summary && summary.units_per_second > 0
					? summary.units_per_second.toFixed(summary.units_per_second < 10 ? 1 : 0)
					: '—'}
			</span>
			<span class="muted">части в секунда</span>
		</div>
		<div class="figure">
			<span class="num" data-testid="network-mine">{mine}</span>
			<span class="muted">{plural(mine, 'част', 'части')} от вас</span>
		</div>
	</div>

	{#if summary?.job}
		<div class="job">
			<div class="job-head">
				<span
					><Icon name={summary.job.template === 'pt' ? 'image' : 'atom'} size={16} />
					{jobName[summary.job.template] ?? summary.job.template}</span
				>
				<span class="muted">{Math.round(progress * 100)} %</span>
			</div>
			<div class="progress"><div style:width="{progress * 100}%"></div></div>
		</div>
	{:else if summary}
		<p class="muted idle">В момента мрежата няма задача.</p>
	{/if}
</section>

<style>
	.network {
		display: grid;
		gap: var(--s-4);
	}
	header {
		display: flex;
		justify-content: space-between;
		align-items: flex-start;
		gap: var(--s-3);
	}
	header .eyebrow {
		margin-bottom: 2px;
	}
	h2 {
		font-size: var(--text-lg);
		margin: 0;
		overflow-wrap: anywhere;
	}
	.figures {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: var(--s-3);
	}
	.figure {
		display: grid;
		gap: 2px;
		font-size: var(--text-xs);
		min-width: 0;
	}
	.num {
		font-size: var(--text-xl);
		font-weight: 750;
		letter-spacing: -0.03em;
		font-variant-numeric: tabular-nums;
		line-height: 1.1;
	}
	.kinds {
		display: flex;
		gap: 8px;
	}
	.kinds span {
		display: inline-flex;
		align-items: center;
		gap: 4px;
	}
	.sw {
		width: 8px;
		height: 8px;
		border-radius: 2px;
	}
	.gpu {
		background: var(--brand);
	}
	.cpu {
		background: #5b8cff;
	}
	.srv {
		background: #a78bfa;
	}
	.job {
		display: grid;
		gap: 6px;
		font-size: var(--text-sm);
	}
	.job-head {
		display: flex;
		justify-content: space-between;
	}
	.job-head span:first-child {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		font-weight: 600;
	}
	.idle {
		margin: 0;
		font-size: var(--text-sm);
	}
</style>
