<script lang="ts">
	import Icon from '#lib/ui/Icon.svelte';

	// A value people copy: a join link, an API key, an invitation link.
	let { value, label, testid }: { value: string; label: string; testid?: string } = $props();
	let copied = $state(false);
	let timer: ReturnType<typeof setTimeout> | undefined;

	async function copy() {
		try {
			await navigator.clipboard.writeText(value);
		} catch {
			// No clipboard (insecure page, old browser): select it for a manual copy.
			input?.select();
			return;
		}
		copied = true;
		clearTimeout(timer);
		timer = setTimeout(() => (copied = false), 1800);
	}
	let input = $state<HTMLInputElement>();
</script>

<div class="copy">
	<input
		readonly
		{value}
		aria-label={label}
		bind:this={input}
		data-testid={testid}
		onfocus={(e) => e.currentTarget.select()}
	/>
	<button type="button" class="secondary" onclick={copy} aria-live="polite">
		<Icon name={copied ? 'check' : 'code'} size={16} />
		{copied ? 'Копирано' : 'Копирай'}
	</button>
</div>

<style>
	.copy {
		display: flex;
		flex-wrap: wrap;
		gap: var(--s-2);
	}
	input {
		flex: 1 1 220px;
		font-family: ui-monospace, 'SF Mono', Menlo, Consolas, monospace;
		font-size: 14px;
		min-width: 0;
	}
	button {
		flex: none;
	}
</style>
