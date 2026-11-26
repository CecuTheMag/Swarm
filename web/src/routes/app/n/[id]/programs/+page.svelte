<script lang="ts">
	// A network's own programs (spec 0012): upload a module, see where it stands in
	// review, ask for review for the public network.
	import { page } from '$app/state';
	import { api, failureText } from '#lib/api.ts';
	import { toBase64 } from '#lib/bytes.ts';
	import { can, useNetwork } from '#lib/console/networks.svelte.ts';
	import type { ProgramSummary } from '#lib/protocol/ProgramSummary.ts';
	import type { Review } from '#lib/protocol/Review.ts';

	const network = useNetwork();
	const id = $derived(page.params.id!);
	const admin = $derived(network.view ? can(network.view.role, 'admin') : false);

	let programs = $state<ProgramSummary[]>([]);
	let error = $state<string | null>(null);

	async function load() {
		try {
			programs = await api<ProgramSummary[]>(`/api/networks/${id}/programs`);
		} catch (e) {
			error = failureText(e);
		}
	}
	$effect(() => {
		void load();
	});

	const reviewText: Record<Review, string> = {
		private: 'само в мрежата',
		pending: 'чака преглед',
		approved: 'одобрена за общата мрежа',
		rejected: 'отказана'
	};

	let name = $state('');
	let description = $state('');
	let sourceUrl = $state('');
	let file = $state<File | null>(null);
	let busy = $state(false);

	async function upload(event: SubmitEvent) {
		event.preventDefault();
		if (!file) return;
		busy = true;
		error = null;
		try {
			await api<ProgramSummary>(`/api/networks/${id}/programs`, {
				body: {
					name,
					description,
					source_url: sourceUrl || undefined,
					module: toBase64(new Uint8Array(await file.arrayBuffer()))
				}
			});
			name = description = sourceUrl = '';
			file = null;
			(event.target as HTMLFormElement).reset();
			await load();
		} catch (e) {
			error = failureText(e);
		} finally {
			busy = false;
		}
	}

	async function askReview(program: string) {
		error = null;
		try {
			await api(`/api/networks/${id}/programs/${program}/review`, { body: {} });
			await load();
		} catch (e) {
			error = failureText(e);
		}
	}
</script>

<svelte:head>
	<title>Програми · {network.view?.name} · Рояк</title>
</svelte:head>

{#if error}<p class="notice bad" role="alert" data-testid="program-error">{error}</p>{/if}

<section class="card">
	<h2>Програми</h2>
	<p class="muted">
		Ваша собствена програма, изпълнявана на устройствата на мрежата: модул на WebAssembly, който
		получава вход и връща резултат за всяка част. Програмата няма достъп до нищо извън паметта си:
		нито до мрежата, нито до файловете на устройството. Затова еднакъв вход дава еднакъв резултат
		навсякъде и резултатите се проверяват, като се сравнява какво са изчислили различни устройства.
	</p>
	{#if programs.length === 0}
		<p class="muted">Още няма програми{admin ? '. Качете първата отдолу.' : '.'}</p>
	{:else}
		<ul class="programs">
			{#each programs as p (p.id)}
				<li data-testid="program" data-id={p.id}>
					<div class="about">
						<strong>{p.name}</strong>
						{#if p.description}<span class="muted">{p.description}</span>{/if}
						<span class="muted small">
							{(p.size / 1024).toFixed(1)} KiB · <code>{p.sha256.slice(0, 12)}</code>
							{#if p.source_url}
								· <a href={p.source_url} rel="external noopener noreferrer">изходен код</a>
							{/if}
						</span>
					</div>
					<div class="review">
						<span
							class="badge"
							class:ok={p.review === 'approved'}
							class:bad={p.review === 'rejected'}
							data-testid="review"
							data-review={p.review}>{reviewText[p.review]}</span
						>
						{#if p.review_note}<span class="muted small">{p.review_note}</span>{/if}
						{#if admin && (p.review === 'private' || p.review === 'rejected')}
							<button class="secondary sm" data-testid="ask-review" onclick={() => askReview(p.id)}
								>Поискай преглед</button
							>
						{/if}
					</div>
				</li>
			{/each}
		</ul>
	{/if}
</section>

{#if admin}
	<section class="card">
		<h2>Качи програма</h2>
		<form class="upload" onsubmit={upload}>
			<label
				>Име <input bind:value={name} required maxlength="80" data-testid="program-name" /></label
			>
			<label>Описание <input bind:value={description} maxlength="2000" /></label>
			<label
				>Изходен код (https://…, за прегледа)
				<input bind:value={sourceUrl} type="url" maxlength="300" /></label
			>
			<label
				>Модул (.wasm)
				<input
					type="file"
					accept=".wasm,application/wasm"
					required
					data-testid="program-file"
					onchange={(e) => (file = e.currentTarget.files?.[0] ?? null)}
				/></label
			>
			<button type="submit" disabled={busy || !file} data-testid="upload">Качи</button>
		</form>
		<p class="muted small">
			Изисквания: без импорти, памет с обявен максимум до 256 MiB, изнесени <code>memory</code>,
			<code>swarm_input</code>, <code>swarm_run</code> и <code>swarm_output</code>. Как се пише
			програма: <code>docs/design/programs.md</code>, пример на Rust в
			<code>examples/programs</code>.
		</p>
	</section>
{/if}

<style>
	section + section {
		margin-top: var(--s-4);
	}
	h2 {
		font-size: var(--text-lg);
		margin: 0 0 var(--s-3);
	}
	.programs {
		list-style: none;
		margin: var(--s-3) 0 0;
		padding: 0;
		display: grid;
		gap: var(--s-2);
	}
	.programs li {
		display: flex;
		flex-wrap: wrap;
		justify-content: space-between;
		gap: var(--s-3);
		padding: var(--s-3) 0;
		border-top: 1px solid var(--border);
	}
	.about,
	.review {
		display: grid;
		gap: 4px;
	}
	.review {
		justify-items: end;
	}
	.upload {
		display: grid;
		gap: var(--s-3);
		grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
		align-items: end;
	}
	.small {
		font-size: var(--text-sm);
	}
</style>
