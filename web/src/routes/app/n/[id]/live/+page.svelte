<script lang="ts">
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { api } from '#lib/api.ts';
	import LiveView from '#lib/dashboard/LiveView.svelte';
	import { can, useNetwork } from '#lib/console/networks.svelte.ts';
	import Icon from '#lib/ui/Icon.svelte';
	import type { JobSummary } from '#lib/protocol/JobSummary.ts';

	// A network's live view for a screen or projector: the job of `?job=`, or the
	// network's latest image.
	const network = useNetwork();
	const id = page.params.id!;
	let jobId = $state<string | null>(page.url.searchParams.get('job'));
	let none = $state(false);

	$effect(() => {
		if (jobId) return;
		void api<JobSummary[]>(`/api/networks/${id}/jobs`).then((jobs) => {
			const latest = jobs.find((j) => j.template === 'pt');
			if (latest) jobId = latest.id;
			else none = true;
		});
	});

	const view = $derived(network.view);
	const join = $derived(
		view && !view.public ? resolve('/join/[code]', { code: view.join_code }) : resolve('/worker')
	);
</script>

<svelte:head>
	<title>На живо · {view?.name ?? 'Рояк'}</title>
</svelte:head>

<a class="back" href={resolve('/app/n/[id]', { id })}
	><Icon name="arrow" size={14} /> Към конзолата</a
>

{#if view && jobId}
	{#key jobId}
		<LiveView
			{jobId}
			jobPath="/api/networks/{id}/jobs/{jobId}"
			stream="/ws/networks/{id}/live"
			{join}
			chaos={can(view.role, 'admin')
				? { base: `/api/networks/${id}/chaos`, password: false }
				: null}
		/>
	{/key}
{:else if none}
	<main class="none">
		<div class="card">
			<h1>Още няма изображение</h1>
			<p class="muted">Пуснете задача „Изображение“ и тя ще се появи тук.</p>
			<a class="button" href={resolve('/app/n/[id]/jobs', { id })}>Към задачите</a>
		</div>
	</main>
{/if}

<style>
	.back {
		position: fixed;
		left: 16px;
		bottom: 16px;
		z-index: 40;
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 8px 14px;
		border-radius: var(--pill);
		background: var(--glass-strong);
		border: 1px solid var(--glass-border);
		color: var(--text);
		text-decoration: none;
		font-size: var(--text-xs);
		backdrop-filter: blur(12px);
	}
	.back :global(svg) {
		transform: rotate(180deg);
	}
	.none {
		max-width: 560px;
	}
</style>
