<script lang="ts">
	import { onMount } from 'svelte';
	import 'swagger-ui-dist/swagger-ui.css';

	let target: HTMLElement;
	let failed = $state(false);

	onMount(async () => {
		try {
			const { default: SwaggerUIBundle } = await import('swagger-ui-dist/swagger-ui-bundle.js');
			SwaggerUIBundle({
				url: '/api/openapi.json',
				domNode: target,
				deepLinking: true,
				docExpansion: 'list',
				defaultModelsExpandDepth: 0,
				// "Try it out" uses the session of this browser, as the console does.
				withCredentials: true
			});
		} catch {
			failed = true;
		}
	});
</script>

<svelte:head>
	<title>Програмен интерфейс · Рояк</title>
</svelte:head>

<main class="api-docs">
	<header class="page-head">
		<span class="eyebrow">За разработчици</span>
		<h1>Програмен интерфейс (REST API)</h1>
		<p>
			Всичко, което прави конзолата, става и през този интерфейс: мрежи, устройства, задачи,
			програми и училищни разписания. Описанието е по стандарта OpenAPI 3.1 и се генерира от самия
			код на координатора, затова винаги отговаря на него. Скриптовете влизат с ключ на мрежата (<code
				>Authorization: Bearer …</code
			>), а „Try it out“ използва вашия вход в сайта.
		</p>
		<p class="small">
			Машинно четимо описание: <a href="/api/openapi.json" rel="external">/api/openapi.json</a>
		</p>
	</header>

	{#if failed}
		<p class="card bad">Описанието не се зареди. Опитайте отново по-късно.</p>
	{/if}
	<section
		class="card sheet"
		bind:this={target}
		aria-label="Описание на програмния интерфейс"
	></section>
</main>

<style>
	.api-docs {
		max-width: 1180px;
		display: grid;
		gap: var(--s-4);
	}
	.small {
		font-size: var(--text-sm);
	}
	/* Swagger UI has one light theme: it keeps it on a light sheet in either mode. */
	.sheet {
		color-scheme: light;
		background: #fff;
		color: #1d1d1f;
		padding: var(--s-2) var(--s-4);
		overflow-x: auto;
	}
	.sheet :global(.swagger-ui .scheme-container) {
		box-shadow: none;
		padding: var(--s-3) 0;
	}
</style>
