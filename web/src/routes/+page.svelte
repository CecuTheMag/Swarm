<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import { LiveDashboard } from '#lib/dashboard/live.svelte.ts';
	import { caughtDevices, unitsPerSecond } from '#lib/dashboard/state.ts';
	import LiveHive from '#lib/ui/LiveHive.svelte';
	import Icon, { type IconName } from '#lib/ui/Icon.svelte';
	import { reveal } from '#lib/ui/reveal.ts';
	import { rate as rateFormat } from '#lib/ui/format.ts';

	// The public face of the project: what it is, how to join, why it is safe. The hero
	// shows the network as it is right now: every cell is a connected device.
	const live = new LiveDashboard();
	onMount(() => live.start());

	// Phones show the safety promises as rows that open on a tap; from 761 px they are
	// a grid that is always open (and open without JavaScript too).
	let wide = $state(true);
	onMount(() => {
		const query = matchMedia('(min-width: 761px)');
		const sync = () => (wide = query.matches);
		sync();
		query.addEventListener('change', sync);
		return () => query.removeEventListener('change', sync);
	});
	function keepOpen(event: Event) {
		const details = event.currentTarget as HTMLDetailsElement;
		if (wide && !details.open) details.open = true;
	}
	onDestroy(() => live.stop());
	const workers = $derived(Object.values(live.state.workers));
	const devices = $derived(workers.length);
	const caught = $derived(new Set(caughtDevices(live.state, live.now).map((c) => c.worker.id)));
	const rate = $derived(unitsPerSecond(live.state, live.now));
	const devicesText = (n: number) => (n === 1 ? '1 устройство' : `${n} устройства`);
	const rateText = $derived(rate > 0 ? rateFormat(rate) : '—');
	const statusText = $derived(
		live.connected
			? devices > 0
				? `${devicesText(devices)} в мрежата сега`
				: 'Мрежата е на линия'
			: 'Свързване с мрежата…'
	);

	const proof = [
		{ value: '1 000', label: 'устройства наведнъж в натоварващите тестове' },
		{ value: '3 с', label: 'до хванат измамник на живо' },
		{ value: '0', label: 'инсталации и лични данни' },
		{ value: '1', label: 'докосване, за да се включите' }
	];

	const steps: { icon: IconName; title: string; text: string }[] = [
		{
			icon: 'phone',
			title: 'Отворете страницата',
			text: 'Сканирайте QR кода или отворете „Включи се“ на телефон или компютър. Нищо не се инсталира.'
		},
		{
			icon: 'bolt',
			title: 'Устройството помага',
			text: 'Получава малки части от голяма задача и ги изчислява с видеокартата, а ако няма такава — с процесора.'
		},
		{
			icon: 'check',
			title: 'Резултатът се проверява',
			text: 'Всяка част се смята поне два пъти от различни устройства. Сглобява се само проверено.'
		}
	];

	const uses: { icon: IconName; title: string; text: string; soon?: boolean }[] = [
		{
			icon: 'image',
			title: 'Реалистични изображения',
			text: 'Проследяване на лъчи: всяко устройство изчислява парче от кадъра, а залата гледа как картината изплува.'
		},
		{
			icon: 'atom',
			title: 'Научни симулации',
			text: 'Метод Монте Карло с точен, повторим резултат — от числото π до физични модели.'
		},
		{
			icon: 'chip',
			title: 'Обучение на невронни мрежи',
			text: 'Търсене на най-добрите настройки за малки модели, по едно обучение на устройство.',
			soon: true
		}
	];

	const features: { icon: IconName; title: string; text: string }[] = [
		{
			icon: 'chip',
			title: 'Своя мрежа',
			text: 'Кабинет, лаборатория или екип: устройствата в нея изчисляват само вашите задачи.'
		},
		{
			icon: 'phone',
			title: 'Устройства с един код',
			text: 'Връзка или QR код. Може да изисквате одобрение за всяко ново устройство.'
		},
		{
			icon: 'pause',
			title: 'Пълен контрол',
			text: 'Пауза на едно или на всички, максимална мощност и работно време, например само след часовете.'
		},
		{
			icon: 'shield',
			title: 'Екип с роли',
			text: 'Собственици, администратори и наблюдатели, поканени с лична връзка.'
		},
		{
			icon: 'code',
			title: 'Програмен интерфейс',
			text: 'Ключове за API: вашите програми пускат задачи и четат резултатите.'
		},
		{
			icon: 'image',
			title: 'На живо',
			text: 'Всяко устройство и всяка задача в реално време, и изглед за голям екран.'
		}
	];

	const safety: { icon: IconName; title: string; text: string }[] = [
		{
			icon: 'power',
			title: 'Вие избирате мощността',
			text: 'От 25 до 100 %. Устройството почива между задачите и не прегрява.'
		},
		{
			icon: 'eye',
			title: 'Спира, когато го оставите',
			text: 'Превключите ли към друго приложение или заключите екрана, изчисленията спират сами.'
		},
		{
			icon: 'battery',
			title: 'Пази батерията',
			text: 'Където браузърът показва батерията, Рояк спира под 30 % и продължава, щом телефонът се зарежда.'
		},
		{
			icon: 'lock',
			title: 'Без лични данни',
			text: 'Не питаме за име, имейл или профил. Пазим само случаен номер на устройството, за да знаем на кого да вярваме.'
		},
		{
			icon: 'shield',
			title: 'Само проверен код',
			text: 'Изпълняват се единствено одобрени изчислителни шаблони, сверени по контролна сума.'
		},
		{
			icon: 'code',
			title: 'Отворен код',
			text: 'Всичко е публично под лиценз GPL-3.0 — всеки може да провери как работи.'
		}
	];
</script>

<svelte:head>
	<title>Рояк — суперкомпютър от устройствата около нас</title>
</svelte:head>

<main class="home">
	<!-- Hero -->
	<section class="hero">
		<div class="hero-copy">
			<span class="badge live tnum copy-status" data-testid="home-live">
				<span class="dot pulse"></span>
				{statusText}
			</span>
			<h1 class="display">
				Суперкомпютър от устройствата <span class="accent">около нас</span>.
			</h1>
			<p class="lead">
				Рояк събира свободната мощ на телефони и компютри направо в браузъра и я превръща в
				изчисления за училища и учени. Включвате се с едно докосване — без инсталиране.
			</p>
			<div class="actions">
				<a class="button lg" href={resolve('/worker')}>
					Включи това устройство <Icon name="arrow" size={18} />
				</a>
				<a class="button lg secondary" href="#organisations">За организации</a>
			</div>
		</div>

		<figure class="hero-visual glass">
			<div class="hive-wrap"><LiveHive {workers} {caught} /></div>
			<!-- Phones: the status sits on the field itself (the copy's badge is hidden). -->
			<span class="badge live tnum field-status">
				<span class="dot pulse"></span>
				{live.connected ? 'На живо' : 'Свързване…'}
			</span>
			<figcaption class="hero-card">
				<div class="hero-stat">
					<span class="num" data-testid="home-devices">{devices}</span>
					<span class="muted">{devices === 1 ? 'устройство' : 'устройства'} на линия</span>
				</div>
				<div class="hero-stat">
					<span class="num">{rateText}</span>
					<span class="muted">изчислени части в секунда</span>
				</div>
			</figcaption>
		</figure>
	</section>

	<!-- Proof -->
	<section class="proof" aria-label="Измерено">
		{#each proof as p, i (p.label)}
			<div class="proof-item" use:reveal={i * 50}>
				<span class="proof-value">{p.value}</span>
				<span class="muted">{p.label}</span>
			</div>
		{/each}
	</section>

	<!-- How -->
	<section id="how" class="section">
		<div class="section-head" use:reveal>
			<h2>Три стъпки. Никакви настройки.</h2>
			<p class="muted">
				Голямата задача се разделя на хиляди малки части. Всяко включено устройство взема по
				няколко, изчислява ги и връща резултата.
			</p>
		</div>
		<ol class="steps">
			{#each steps as s, i (s.title)}
				<li class="step" use:reveal={i * 70}>
					<span class="step-no" aria-hidden="true">{i + 1}</span>
					<h3>{s.title}</h3>
					<p class="muted">{s.text}</p>
				</li>
			{/each}
		</ol>
	</section>

	<!-- Uses -->
	<section class="section">
		<div class="section-head" use:reveal>
			<h2>Истинска работа, а не демонстрация.</h2>
		</div>
		<div class="uses">
			{#each uses as u, i (u.title)}
				<article class="card use" use:reveal={i * 70}>
					<Icon name={u.icon} size={22} />
					<h3>
						{u.title}
						{#if u.soon}<span class="badge">скоро</span>{/if}
					</h3>
					<p class="muted">{u.text}</p>
				</article>
			{/each}
		</div>
	</section>

	<!-- Safety -->
	<section id="safety" class="section">
		<div class="safety card" use:reveal>
			<div class="section-head">
				<h2>Вашето устройство остава ваше.</h2>
				<p class="muted">
					Рояк взема само толкова, колкото позволите, и спира в момента, в който поискате.
				</p>
			</div>
			<ul class="safety-grid">
				{#each safety as s (s.title)}
					<li>
						<details open={wide} ontoggle={keepOpen}>
							<summary>
								<Icon name={s.icon} size={20} />
								<h3>{s.title}</h3>
								<span class="chev" aria-hidden="true"><Icon name="chevron" size={18} /></span>
							</summary>
							<p class="muted">{s.text}</p>
						</details>
					</li>
				{/each}
			</ul>
		</div>
	</section>

	<!-- Trust -->
	<section class="section split">
		<div use:reveal>
			<h2>Никой не може да подправи резултата.</h2>
			<p class="muted">
				Всеки може да се включи — значи всеки може и да опита да мами. Затова всяка част се
				изчислява от поне две независими устройства, понякога получават задачи с вече известен
				отговор, а онези, които грешат, губят доверие и биват блокирани. Работата им се изчислява
				наново.
			</p>
		</div>
		<div class="card verdict" use:reveal={100}>
			<div class="verdict-row">
				<span class="badge ok"><Icon name="check" size={14} /> съвпада</span>
				<span>Телефон · Видеокарта</span>
			</div>
			<div class="verdict-row">
				<span class="badge ok"><Icon name="check" size={14} /> съвпада</span>
				<span>Лаптоп · Процесор</span>
			</div>
			<div class="verdict-row caught">
				<span class="badge bad"><Icon name="close" size={14} /> блокиран</span>
				<span>Измамник · 3 грешни резултата</span>
			</div>
			<p class="muted small">Така изглежда проверката на живо по време на демонстрация.</p>
		</div>
	</section>

	<!-- Organisations -->
	<section id="organisations" class="section">
		<div class="section-head" use:reveal>
			<h2>Вашата собствена мрежа от устройства.</h2>
			<p class="muted">
				Училища, лаборатории и изследователски екипи създават своя мрежа, добавят устройства и ги
				управляват от една конзола.
			</p>
		</div>
		<ul class="features" use:reveal>
			{#each features as f (f.title)}
				<li>
					<h3><Icon name={f.icon} size={18} /> {f.title}</h3>
					<p class="muted">{f.text}</p>
				</li>
			{/each}
		</ul>
		<div class="org-cta" use:reveal>
			<a class="button lg" href={resolve('/login')}
				>Вход в конзолата <Icon name="arrow" size={18} /></a
			>
			<span class="muted">Достъпът е с покана от администратора на платформата.</span>
		</div>
	</section>

	<!-- Schools -->
	<section class="section">
		<div class="schools card" use:reveal>
			<Icon name="school" size={26} />
			<div>
				<h2>Компютърният кабинет работи и след последния час.</h2>
				<p class="muted">
					Хиляди училищни компютри стоят изключени по-голямата част от денонощието. Рояк ги превръща
					в обща изчислителна мрежа — без администраторски права и без да се инсталира нищо.
				</p>
			</div>
		</div>
	</section>

	<!-- CTA -->
	<section class="section">
		<div class="cta" use:reveal>
			<h2>Включете се сега.</h2>
			<p class="muted">Отнема секунда. Можете да спрете по всяко време.</p>
			<a class="button lg" href={resolve('/worker')}>
				Включи това устройство <Icon name="arrow" size={18} />
			</a>
		</div>
	</section>
	<!-- Phones: what the footer holds elsewhere -->
	<nav class="more" aria-label="Още">
		<h2>Още</h2>
		<ul class="rows">
			<li>
				<a href={resolve('/join')}
					><Icon name="key" size={20} /><span>Включи се с код</span><Icon
						name="chevron"
						size={18}
					/></a
				>
			</li>
			<li>
				<a href={resolve('/selftest')}
					><Icon name="check" size={20} /><span>Самопроверка</span><Icon
						name="chevron"
						size={18}
					/></a
				>
			</li>
			<li>
				<a href={resolve('/app')}
					><Icon name="grid" size={20} /><span>Конзола</span><Icon name="chevron" size={18} /></a
				>
			</li>
		</ul>
		<p class="muted fine">
			Доброволен суперкомпютър в браузъра. Отворен код, без инсталиране, без лични данни. ©
			{new Date().getFullYear()} Рояк · GPL-3.0
		</p>
	</nav>
</main>

<style>
	.home {
		padding-top: var(--s-5);
	}

	/* ---------- Hero ---------- */
	.hero {
		display: grid;
		gap: var(--s-7);
		align-items: center;
		grid-template-columns: minmax(0, 1.05fr) minmax(0, 1fr);
		padding: var(--s-7) 0 var(--s-8);
	}
	.hero-copy {
		animation: enter var(--slow) var(--ease) both;
	}
	.display {
		font-size: var(--text-display);
		line-height: 1.02;
		letter-spacing: -0.04em;
		margin: var(--s-4) 0 var(--s-5);
	}
	.accent {
		color: var(--grad-a);
	}
	.lead {
		font-size: clamp(1.0625rem, 1rem + 0.4vw, 1.3rem);
		line-height: 1.5;
		color: var(--muted);
		max-width: 34em;
	}
	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: var(--s-3);
		margin-top: var(--s-6);
	}
	.hero-visual {
		position: relative;
		margin: 0;
		aspect-ratio: 5 / 4;
		border-radius: var(--radius-xl);
		overflow: hidden;
		animation: enter var(--slow) var(--ease) 80ms both;
	}
	.hive-wrap {
		position: absolute;
		inset: 0;
		mask-image: radial-gradient(90% 85% at 50% 40%, #000 50%, transparent 100%);
	}
	.hero-card {
		position: absolute;
		left: var(--s-4);
		right: var(--s-4);
		bottom: var(--s-4);
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: var(--s-4);
		padding: var(--s-4) var(--s-5);
		border-radius: var(--radius);
		background: var(--glass-strong);
		border: 1px solid var(--glass-border);
		box-shadow:
			inset 0 1px 0 var(--glass-highlight),
			var(--shadow-sm);
	}
	.hero-stat {
		display: grid;
		gap: 2px;
		min-width: 0;
		font-size: var(--text-xs);
	}
	.num {
		font-size: clamp(1.6rem, 1.2rem + 1.4vw, 2.25rem);
		font-weight: 700;
		letter-spacing: -0.03em;
		font-variant-numeric: tabular-nums;
		line-height: 1.05;
	}
	@keyframes enter {
		from {
			opacity: 0;
			transform: translateY(12px);
		}
	}

	/* ---------- Proof ---------- */
	.proof {
		display: grid;
		grid-template-columns: repeat(4, minmax(0, 1fr));
		gap: var(--s-5);
		padding: var(--s-6) 0;
		border-top: 1px solid var(--border);
		border-bottom: 1px solid var(--border);
	}
	.proof-item {
		display: grid;
		align-content: start;
		gap: 6px;
		font-size: var(--text-sm);
		line-height: 1.4;
	}
	.proof-value {
		font-size: var(--text-xl);
		font-weight: 700;
		letter-spacing: -0.03em;
		line-height: 1.1;
		font-variant-numeric: tabular-nums;
	}

	/* ---------- Sections ---------- */
	.section {
		padding-top: var(--s-9);
	}
	.section-head {
		max-width: 640px;
		margin-bottom: var(--s-6);
	}
	.section-head h2,
	.split h2,
	.cta h2 {
		font-size: var(--text-2xl);
		letter-spacing: -0.035em;
		margin-bottom: var(--s-4);
	}
	.section-head p {
		font-size: var(--text-lg);
		line-height: 1.5;
		margin: 0;
	}

	/* The three steps: a sequence on one rule, not three boxes. */
	.steps {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: var(--s-6);
		border-top: 1px solid var(--border-strong);
	}
	.step {
		position: relative;
		padding-top: var(--s-6);
	}
	.step-no {
		position: absolute;
		top: -14px;
		left: 0;
		display: grid;
		place-items: center;
		width: 28px;
		height: 28px;
		border-radius: 50%;
		background: var(--bg);
		border: 1px solid var(--border-strong);
		color: var(--brand-ink);
		font-size: var(--text-xs);
		font-weight: 700;
		font-variant-numeric: tabular-nums;
	}
	.step h3 {
		margin-bottom: var(--s-2);
	}
	.step p,
	.use p {
		margin: 0;
	}

	.uses {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: var(--s-4);
	}
	.use {
		display: grid;
		align-content: start;
		gap: var(--s-2);
		padding: var(--s-6) var(--s-5);
	}
	.use > :global(svg),
	.schools > :global(svg) {
		color: var(--brand-ink);
		margin-bottom: var(--s-3);
	}
	.use h3 {
		display: flex;
		align-items: center;
		gap: var(--s-2);
		flex-wrap: wrap;
		margin: 0;
	}

	.safety {
		padding: clamp(var(--s-5), 5vw, var(--s-8));
		border-radius: var(--radius-xl);
	}
	.safety-grid {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: var(--s-6) var(--s-6);
	}
	.safety-grid summary {
		display: flex;
		gap: var(--s-3);
		align-items: flex-start;
		list-style: none;
	}
	.safety-grid summary::-webkit-details-marker {
		display: none;
	}
	.safety-grid summary > :global(svg) {
		flex: none;
		margin-top: 2px;
		color: var(--brand-ink);
	}
	.safety-grid details > p {
		padding-left: 32px;
	}
	.chev {
		display: none;
	}
	.more,
	.field-status {
		display: none;
	}
	/* From 761 px the promises are always open: the summary is a plain heading. */
	@media (min-width: 761px) {
		.safety-grid summary {
			pointer-events: none;
		}
	}
	.safety-grid h3,
	.features h3 {
		font-size: var(--text-md);
		letter-spacing: -0.01em;
		margin-bottom: 4px;
	}
	.safety-grid p,
	.features p {
		margin: 0;
		font-size: var(--text-sm);
		line-height: 1.5;
	}

	.split {
		display: grid;
		grid-template-columns: minmax(0, 1.1fr) minmax(0, 1fr);
		gap: var(--s-7);
		align-items: center;
	}
	.split p {
		font-size: var(--text-lg);
		line-height: 1.55;
	}
	.verdict {
		display: grid;
		padding: var(--s-3) var(--s-5) var(--s-4);
	}
	.verdict-row {
		display: flex;
		align-items: center;
		gap: var(--s-3);
		padding: var(--s-4) 0;
		border-bottom: 1px solid var(--border);
		font-size: var(--text-sm);
		font-weight: 500;
	}
	.verdict-row.caught {
		color: var(--bad);
	}
	.verdict .small {
		font-size: var(--text-xs);
		line-height: 1.45;
		margin: var(--s-3) 0 0;
	}

	/* What an organisation gets: a list on hairlines. */
	.features {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		column-gap: var(--s-6);
	}
	.features li {
		padding: var(--s-5) 0;
		border-top: 1px solid var(--border);
	}
	.features h3 {
		display: flex;
		align-items: center;
		gap: var(--s-2);
	}
	.features h3 :global(svg) {
		flex: none;
		color: var(--brand-ink);
	}

	.org-cta {
		display: flex;
		align-items: center;
		gap: var(--s-4);
		flex-wrap: wrap;
		margin-top: var(--s-5);
	}
	.schools {
		display: flex;
		gap: var(--s-5);
		align-items: flex-start;
		padding: clamp(var(--s-5), 5vw, var(--s-7));
		border-radius: var(--radius-xl);
	}
	.schools > :global(svg) {
		flex: none;
		margin: 6px 0 0;
	}
	.schools h2 {
		font-size: var(--text-xl);
	}
	.schools p {
		margin: 0;
		max-width: 60ch;
	}

	.cta {
		text-align: center;
		padding: clamp(var(--s-6), 7vw, var(--s-9)) var(--s-5);
	}
	.cta p {
		font-size: var(--text-lg);
		margin-bottom: var(--s-5);
	}

	/* ---------- Narrow screens ---------- */
	@media (max-width: 980px) {
		.hero {
			grid-template-columns: 1fr;
			gap: var(--s-6);
			padding-top: var(--s-5);
		}
		.hero-visual {
			aspect-ratio: 4 / 3;
		}
		.split {
			grid-template-columns: 1fr;
			gap: var(--s-5);
		}
		.safety-grid,
		.features {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}
	}
	/* ---------- Phones: „Рояк в джоба“ (docs/design-system.md) ---------- */
	@media (max-width: 760px) {
		.home {
			padding-top: 0;
		}

		/* The swarm is the screen: the live field first, edge to edge. */
		.hero {
			display: flex;
			flex-direction: column;
			align-items: stretch;
			gap: var(--s-5);
			padding: 0;
		}
		.hero-visual {
			order: -1;
			aspect-ratio: auto;
			height: min(56svh, 470px);
			margin: 0 calc(-1 * var(--gutter));
			border-radius: 0 0 var(--radius-xl) var(--radius-xl);
			border-top: 0;
			border-left: 0;
			border-right: 0;
		}
		.hive-wrap {
			mask-image: linear-gradient(#000 70%, transparent 100%);
		}
		.copy-status {
			display: none;
		}
		.field-status {
			display: inline-flex;
			position: absolute;
			top: var(--s-4);
			left: var(--gutter);
			background: var(--glass-strong);
			border: 1px solid var(--glass-border);
			-webkit-backdrop-filter: blur(12px);
			backdrop-filter: blur(12px);
		}
		.hero-card {
			left: var(--gutter);
			right: var(--gutter);
			bottom: var(--s-4);
			padding: var(--s-4);
			gap: var(--s-3);
		}
		.num {
			font-size: 2.5rem;
		}
		.display {
			font-size: 2.125rem;
			margin: var(--s-3) 0 var(--s-3);
		}
		.lead {
			font-size: var(--text-md);
			margin: 0;
		}
		.actions {
			margin-top: var(--s-5);
			display: grid;
			gap: var(--s-2);
		}
		.actions .button {
			width: 100%;
		}
		.actions .secondary {
			background: transparent;
			border-color: transparent;
			box-shadow: none;
			color: var(--brand-ink);
		}

		/* Rails: swipe sideways, the next card peeking; the page itself never scrolls
		   sideways. */
		.proof,
		.steps,
		.uses {
			display: flex;
			gap: var(--s-3);
			margin: 0 calc(-1 * var(--gutter));
			padding: 0 var(--gutter);
			overflow-x: auto;
			overscroll-behavior-x: contain;
			scroll-snap-type: x mandatory;
			scroll-padding-inline: var(--gutter);
			scrollbar-width: none;
		}
		.proof::-webkit-scrollbar,
		.steps::-webkit-scrollbar,
		.uses::-webkit-scrollbar {
			display: none;
		}
		.proof > *,
		.steps > *,
		.uses > * {
			flex: 0 0 auto;
			scroll-snap-align: start;
		}

		.proof {
			margin-top: var(--s-6);
			padding-top: 0;
			padding-bottom: var(--s-1);
			border: 0;
		}
		.proof-item {
			width: 9.5rem;
			padding: var(--s-4);
			border-radius: var(--radius);
			background: var(--bg-elevated);
			border: 1px solid var(--border);
			font-size: var(--text-xs);
		}
		.proof-value {
			font-size: 1.75rem;
		}

		.section {
			padding-top: var(--s-7);
		}
		.section-head {
			margin-bottom: var(--s-4);
		}
		.section-head h2,
		.split h2,
		.cta h2 {
			font-size: 1.75rem;
			margin-bottom: var(--s-2);
		}
		.section-head p,
		.split p {
			font-size: var(--text-md);
		}

		.steps {
			border: 0;
			list-style: none;
		}
		.step {
			width: 80%;
			padding: var(--s-5);
			border-radius: var(--radius-lg);
			background: var(--bg-elevated);
			border: 1px solid var(--border);
			box-shadow: var(--shadow-sm);
		}
		.step-no {
			position: static;
			width: 32px;
			height: 32px;
			margin-bottom: var(--s-6);
			background: var(--brand);
			border: 0;
			color: var(--on-brand);
			font-size: var(--text-sm);
		}
		.uses .use {
			width: 80%;
			padding: var(--s-5);
		}

		/* Safety: a grouped list; each promise opens on a tap. */
		.safety {
			padding: var(--s-5) 0 var(--s-2);
			border-radius: var(--radius-lg);
		}
		.safety .section-head {
			padding: 0 var(--s-5);
		}
		.safety-grid {
			display: block;
		}
		.safety-grid li {
			border-top: 1px solid var(--border);
		}
		.safety-grid summary {
			align-items: center;
			min-height: 56px;
			padding: var(--s-3) var(--s-5);
			cursor: pointer;
			-webkit-tap-highlight-color: transparent;
			transition: background-color var(--fast) var(--ease);
		}
		.safety-grid summary:active {
			background: rgb(118 118 128 / 0.1);
		}
		.safety-grid summary > :global(svg) {
			margin: 0;
		}
		.safety-grid h3 {
			flex: 1;
			margin: 0;
		}
		.chev {
			display: inline-flex;
			color: var(--faint);
			transition: transform var(--base) var(--ease);
		}
		details[open] .chev {
			transform: rotate(90deg);
		}
		.safety-grid details > p {
			padding: 0 var(--s-5) var(--s-4) calc(var(--s-5) + 32px);
		}

		.split {
			grid-template-columns: 1fr;
			gap: var(--s-4);
		}

		/* What an organisation gets: an inset grouped list. */
		.features {
			display: block;
			background: var(--bg-elevated);
			border: 1px solid var(--border);
			border-radius: var(--radius-lg);
		}
		.features li {
			padding: var(--s-4) var(--s-5);
		}
		.features li:first-child {
			border-top: 0;
		}
		.org-cta .button {
			width: 100%;
		}

		.schools {
			flex-direction: column;
			gap: var(--s-2);
			padding: var(--s-5);
		}
		.schools h2 {
			font-size: var(--text-lg);
		}
		.cta {
			padding: var(--s-6) 0 0;
		}
		.cta .button {
			width: 100%;
		}

		/* The footer's links, as a native list. */
		.more {
			display: block;
			padding-top: var(--s-7);
		}
		.more h2 {
			font-size: var(--text-sm);
			font-weight: 600;
			letter-spacing: 0;
			color: var(--muted);
			margin: 0 0 var(--s-2) var(--s-4);
		}
		.rows {
			list-style: none;
			margin: 0;
			padding: 0;
			background: var(--bg-elevated);
			border: 1px solid var(--border);
			border-radius: var(--radius-lg);
			overflow: hidden;
		}
		.rows li + li {
			border-top: 1px solid var(--border);
		}
		.rows a {
			display: flex;
			align-items: center;
			gap: var(--s-3);
			min-height: 52px;
			padding: 0 var(--s-4);
			color: var(--text);
			text-decoration: none;
			-webkit-tap-highlight-color: transparent;
			transition: background-color var(--fast) var(--ease);
		}
		.rows a:active {
			background: rgb(118 118 128 / 0.1);
		}
		.rows a > :global(svg:first-child) {
			color: var(--brand-ink);
		}
		.rows a > :global(svg:last-child) {
			color: var(--faint);
		}
		.rows span {
			flex: 1;
		}
		.fine {
			margin: var(--s-4) var(--s-4) 0;
			font-size: var(--text-xs);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.hero-copy,
		.hero-visual {
			animation: none;
		}
	}
</style>
