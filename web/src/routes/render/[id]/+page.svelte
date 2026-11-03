<script lang="ts">
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import LiveView from '#lib/dashboard/LiveView.svelte';

	// A public network job's live view.
	const id = page.params.id!;
</script>

<svelte:head>
	<title>На живо · Рояк</title>
</svelte:head>

<LiveView
	jobId={id}
	jobPath="/api/jobs/{id}"
	stream="/ws/dashboard"
	join={resolve('/worker')}
	chaos={{ base: '/api/chaos', password: true }}
/>
