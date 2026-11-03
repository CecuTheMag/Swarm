<script lang="ts">
	// A destructive action in two taps: the first asks, the second does it. Nothing is
	// lost to a stray click, and there is no dialog to dismiss.
	let {
		label,
		confirm,
		onconfirm,
		testid,
		size = 'sm'
	}: {
		label: string;
		confirm: string;
		onconfirm: () => void | Promise<void>;
		testid?: string;
		size?: 'sm' | 'md';
	} = $props();
	let asking = $state(false);
	let timer: ReturnType<typeof setTimeout> | undefined;

	async function click() {
		if (!asking) {
			asking = true;
			clearTimeout(timer);
			timer = setTimeout(() => (asking = false), 4000);
			return;
		}
		asking = false;
		await onconfirm();
	}
</script>

<button
	type="button"
	class={asking ? 'danger' : 'secondary'}
	class:sm={size === 'sm'}
	onclick={click}
	data-testid={testid}
	aria-live="polite"
>
	{asking ? confirm : label}
</button>
