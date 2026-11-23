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
	onDestroy(() => live.stop());
	const workers = $derived(Object.values(live.state.workers));
	const devices = $derived(workers.length);
	const caught = $derived(new Set(caughtDevices(live.state, live.now).map((c) => c.worker.id)));
	const rate = $derived(unitsPerSecond(live.state, live.now));
	const rateText = $derived(rate > 0 ? rateFormat(rate) : '—');

	const devicesText = (n: number) => (n === 1 ? '1 устройство' : `${n} устройства`);

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
			<span class="badge live tnum" data-testid="home-live">
				<span class="dot pulse"></span>
				{live.connected
					? devices > 0
						? `${devicesText(devices)} в мрежата сега`
						: 'Мрежата е на линия'
					: 'Свързване с мрежата…'}
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
						<Icon name={s.icon} size={20} />
						<div>
							<h3>{s.title}</h3>
							<p class="muted">{s.text}</p>
						</div>
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
	.safety-grid li {
		display: flex;
		gap: var(--s-3);
		align-items: flex-start;
	}
	.safety-grid li > :global(svg) {
		flex: none;
		margin-top: 2px;
		color: var(--brand-ink);
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
	@media (max-width: 760px) {
		.proof {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}
		.uses,
		.safety-grid,
		.features {
			grid-template-columns: 1fr;
		}
		/* The sequence turns vertical: the rule runs down the left, numbers on it. */
		.steps {
			grid-template-columns: 1fr;
			gap: var(--s-5);
			border-top: 0;
			border-left: 1px solid var(--border-strong);
			margin-left: 14px;
		}
		.step {
			padding: 0 0 0 var(--s-5);
		}
		.step-no {
			top: 0;
			left: -14px;
		}
		.section {
			padding-top: var(--s-8);
		}
		.schools {
			flex-direction: column;
			gap: var(--s-2);
		}
	}
	@media (max-width: 420px) {
		.actions .button {
			width: 100%;
		}
		.hero-visual {
			aspect-ratio: 1;
		}
		.hero-card {
			left: var(--s-3);
			right: var(--s-3);
			bottom: var(--s-3);
			padding: var(--s-3) var(--s-4);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.hero-copy,
		.hero-visual {
			animation: none;
		}
	}
</style>
