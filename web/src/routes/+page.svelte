<script lang="ts">
	import { resolve } from '$app/paths';
</script>

<main>
	<h1>Рояк</h1>
	<p class="muted">
		Доброволен суперкомпютър от неизползваните училищни и лични устройства — направо в браузъра,
		чрез видеокартата (WebGPU).
	</p>
	<div class="grid">
		<a class="card link" href={resolve('/worker')}>
			<h2>Включи се</h2>
			<p class="muted">Дари изчислителна мощ от това устройство. Без инсталиране.</p>
		</a>
		<a class="card link" href={resolve('/dashboard')}>
			<h2>Табло</h2>
			<p class="muted">Устройствата в мрежата и задачите в реално време.</p>
		</a>
		<a class="card link" href={resolve('/selftest')}>
			<h2>Самопроверка</h2>
			<p class="muted">Проверява дали видеокартата смята точно и колко бързо.</p>
		</a>
	</div>
</main>

<style>
	.link {
		color: inherit;
		text-decoration: none;
	}
	.link:hover {
		border-color: var(--accent);
	}
</style>
