<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';

	// The organiser's dashboard became the console (spec 0010); old links still work.
	onMount(() => goto(resolve('/app'), { replaceState: true }));
</script>

<main><p class="muted">Към конзолата…</p></main>
