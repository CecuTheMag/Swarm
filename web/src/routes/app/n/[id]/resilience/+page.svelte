<script lang="ts">
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { LiveDashboard } from '#lib/dashboard/live.svelte.ts';
	import ChaosPanel from '#lib/dashboard/ChaosPanel.svelte';
	import SwarmMap from '#lib/dashboard/SwarmMap.svelte';
	import { caughtDevices } from '#lib/dashboard/state.ts';
	import { useNetwork } from '#lib/console/networks.svelte.ts';

	// Resilience tests (spec 0008) on this network: show that it survives devices that
	// vanish and devices that lie.
	const network = useNetwork();
	const id = $derived(page.params.id!);
	let live = $state<LiveDashboard | null>(null);
	$effect(() => {
		const l = new LiveDashboard(undefined, `/ws/networks/${id}/live`);
		l.start();
		live = l;
		return () => l.stop();
	});
	const workers = $derived(live ? Object.values(live.state.workers) : []);
	const caught = $derived(live ? caughtDevices(live.state, live.now) : []);
	const caughtIds = $derived(new Set(live ? Object.keys(live.state.caught) : []));
	const totals = $derived(
		live?.state.totals ?? { banned: 0, wrongResults: 0, unitsRedone: 0, left: 0, unitsReturned: 0 }
	);
</script>

<svelte:head>
	<title>Устойчивост · {network.view?.name} · Рояк</title>
</svelte:head>

<div class="grid-2">
	<section class="card">
		<h2>Тест на устойчивостта</h2>
		<p class="muted">
			Проверете на живо, че мрежата издържа: изключете или прекъснете устройство и задачата
			продължава; добавете измамници и вижте как проверката ги хваща. Най-добре докато върви задача,
			отворена на <a href={resolve('/app/n/[id]/live', { id })}>цял екран</a>.
		</p>
		<ChaosPanel {workers} {caughtIds} {totals} base="/api/networks/{id}/chaos" password={false} />
	</section>
	<section class="card">
		<h2>Устройствата сега</h2>
		<SwarmMap {workers} {caught} />
	</section>
</div>

<style>
	.grid-2 {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
		gap: var(--s-4);
		align-items: start;
	}
	h2 {
		font-size: var(--text-lg);
	}
	@media (max-width: 900px) {
		.grid-2 {
			grid-template-columns: minmax(0, 1fr);
		}
	}
</style>
