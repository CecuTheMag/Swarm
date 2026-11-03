<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import { LiveDashboard } from '#lib/dashboard/live.svelte.ts';
	import { unitsPerSecond } from '#lib/dashboard/state.ts';
	import HeroHive from '#lib/ui/HeroHive.svelte';
	import Icon, { type IconName } from '#lib/ui/Icon.svelte';
	import { reveal } from '#lib/ui/reveal.ts';

	// The public face of the project: what it is, how to join, why it is safe. The hero
	// shows the grid as it is right now.
	const live = new LiveDashboard();
	onMount(() => live.start());
	onDestroy(() => live.stop());
	const devices = $derived(Object.keys(live.state.workers).length);
	const rate = $derived(unitsPerSecond(live.state, live.now));

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
			<span class="badge live" data-testid="home-live">
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

		<div class="hero-visual glass">
			<div class="hive-wrap"><HeroHive /></div>
			<div class="hero-card glass">
				<div class="hero-stat">
					<span class="num" data-testid="home-devices">{devices}</span>
					<span class="muted">{devices === 1 ? 'устройство' : 'устройства'} на линия</span>
				</div>
				<div class="hero-stat">
					<span class="num">{rate > 0 ? rate.toFixed(rate < 10 ? 1 : 0) : '—'}</span>
					<span class="muted">изчислени части в секунда</span>
				</div>
			</div>
		</div>
	</section>

	<!-- Proof -->
	<section class="proof" aria-label="Измерено">
		{#each proof as p, i (p.label)}
			<div class="proof-item" use:reveal={i * 70}>
				<span class="proof-value">{p.value}</span>
				<span class="muted">{p.label}</span>
			</div>
		{/each}
	</section>

	<!-- How -->
	<section id="how" class="section">
		<div class="section-head" use:reveal>
			<span class="eyebrow">Как работи</span>
			<h2>Три стъпки. Никакви настройки.</h2>
			<p class="muted">
				Голямата задача се разделя на хиляди малки части. Всяко включено устройство взема по
				няколко, изчислява ги и връща резултата.
			</p>
		</div>
		<ol class="steps">
			{#each steps as s, i (s.title)}
				<li class="card step" use:reveal={i * 90}>
					<span class="step-no">{i + 1}</span>
					<span class="icon-tile"><Icon name={s.icon} size={22} /></span>
					<h3>{s.title}</h3>
					<p class="muted">{s.text}</p>
				</li>
			{/each}
		</ol>
	</section>

	<!-- Uses -->
	<section class="section">
		<div class="section-head" use:reveal>
			<span class="eyebrow">Какво изчисляваме</span>
			<h2>Истинска работа, а не демонстрация.</h2>
		</div>
		<div class="uses">
			{#each uses as u, i (u.title)}
				<article class="card use" use:reveal={i * 90}>
					<span class="icon-tile"><Icon name={u.icon} size={22} /></span>
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
		<div class="safety glass" use:reveal>
			<div class="section-head">
				<span class="eyebrow">Безопасност</span>
				<h2>Вашето устройство остава ваше.</h2>
				<p class="muted">
					Рояк взема само толкова, колкото позволите, и спира в момента, в който поискате.
				</p>
			</div>
			<ul class="safety-grid">
				{#each safety as s, i (s.title)}
					<li use:reveal={i * 60}>
						<span class="icon-tile sm"><Icon name={s.icon} size={20} /></span>
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
			<span class="eyebrow">Честни резултати</span>
			<h2>Никой не може да подправи резултата.</h2>
			<p class="muted">
				Всеки може да се включи — значи всеки може и да опита да мами. Затова всяка част се
				изчислява от поне две независими устройства, понякога получават задачи с вече известен
				отговор, а онези, които грешат, губят доверие и биват блокирани. Работата им се изчислява
				наново.
			</p>
		</div>
		<div class="card verdict" use:reveal={120}>
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
			<span class="eyebrow">За организации</span>
			<h2>Вашата собствена мрежа от устройства.</h2>
			<p class="muted">
				Училища, лаборатории и изследователски екипи създават своя мрежа, добавят устройства и ги
				управляват от една конзола.
			</p>
		</div>
		<div class="uses">
			{#each features as f, i (f.title)}
				<article class="card use" use:reveal={i * 60}>
					<span class="icon-tile"><Icon name={f.icon} size={22} /></span>
					<h3>{f.title}</h3>
					<p class="muted">{f.text}</p>
				</article>
			{/each}
		</div>
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
			<span class="icon-tile"><Icon name="school" size={24} /></span>
			<div>
				<span class="eyebrow">За училищата</span>
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
		<div class="cta glass" use:reveal>
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
		animation: enter var(--slower) var(--ease) both;
	}
	.display {
		font-size: var(--text-display);
		line-height: 1.02;
		letter-spacing: -0.045em;
		margin: var(--s-4) 0 var(--s-5);
	}
	.accent {
		background: linear-gradient(100deg, var(--grad-a), var(--grad-b));
		-webkit-background-clip: text;
		background-clip: text;
		color: transparent;
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
		aspect-ratio: 5 / 4;
		border-radius: var(--radius-xl);
		overflow: hidden;
		animation: enter var(--slower) var(--ease) 120ms both;
	}
	.hive-wrap {
		position: absolute;
		inset: 0;
		mask-image: radial-gradient(85% 85% at 50% 45%, #000 45%, transparent 100%);
	}
	.hero-card {
		position: absolute;
		left: var(--s-5);
		right: var(--s-5);
		bottom: var(--s-5);
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: var(--s-4);
		padding: var(--s-4) var(--s-5);
		border-radius: var(--radius-lg);
		background: var(--glass-strong);
	}
	.hero-stat {
		display: grid;
		gap: 2px;
		font-size: var(--text-xs);
	}
	.num {
		font-size: clamp(1.6rem, 1.2rem + 1.4vw, 2.25rem);
		font-weight: 750;
		letter-spacing: -0.035em;
		font-variant-numeric: tabular-nums;
		line-height: 1.05;
	}
	@keyframes enter {
		from {
			opacity: 0;
			transform: translateY(20px);
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
		gap: 4px;
		font-size: var(--text-sm);
	}
	.proof-value {
		font-size: var(--text-xl);
		font-weight: 750;
		letter-spacing: -0.035em;
	}

	/* ---------- Sections ---------- */
	.section {
		padding-top: var(--s-9);
	}
	.section-head {
		max-width: 640px;
		margin-bottom: var(--s-6);
	}
	.section-head h2 {
		font-size: var(--text-2xl);
		letter-spacing: -0.035em;
	}
	.section-head p {
		font-size: var(--text-lg);
		line-height: 1.5;
	}
	.icon-tile {
		display: inline-grid;
		place-items: center;
		width: 48px;
		height: 48px;
		border-radius: 14px;
		background: var(--brand-soft);
		color: var(--brand-ink);
		margin-bottom: var(--s-4);
		flex: none;
	}
	.icon-tile.sm {
		width: 40px;
		height: 40px;
		border-radius: 12px;
		margin: 0;
	}

	.steps {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: var(--s-4);
	}
	.step,
	.use {
		position: relative;
		padding: var(--s-6) var(--s-5);
		transition:
			transform var(--base) var(--ease),
			box-shadow var(--base) var(--ease);
	}
	.step:hover,
	.use:hover {
		transform: translateY(-4px);
		box-shadow:
			inset 0 1px 0 var(--glass-highlight),
			var(--shadow-lg);
	}
	.step p,
	.use p {
		margin: 0;
	}
	.step-no {
		position: absolute;
		top: var(--s-5);
		right: var(--s-5);
		font-size: 3rem;
		font-weight: 800;
		line-height: 1;
		letter-spacing: -0.05em;
		color: rgb(118 118 128 / 0.14);
	}

	.uses {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: var(--s-4);
	}
	.use h3 {
		display: flex;
		align-items: center;
		gap: var(--s-2);
		flex-wrap: wrap;
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
		gap: var(--s-6) var(--s-5);
	}
	.safety-grid li {
		display: flex;
		gap: var(--s-4);
		align-items: flex-start;
	}
	.safety-grid h3 {
		font-size: var(--text-md);
		margin-bottom: 4px;
	}
	.safety-grid p {
		margin: 0;
		font-size: var(--text-sm);
	}

	.split {
		display: grid;
		grid-template-columns: minmax(0, 1.1fr) minmax(0, 1fr);
		gap: var(--s-7);
		align-items: center;
	}
	.split h2 {
		font-size: var(--text-2xl);
		letter-spacing: -0.035em;
	}
	.split p {
		font-size: var(--text-lg);
		line-height: 1.55;
	}
	.verdict {
		display: grid;
		gap: var(--s-3);
		padding: var(--s-5);
	}
	.verdict-row {
		display: flex;
		align-items: center;
		gap: var(--s-3);
		padding: var(--s-3) var(--s-4);
		border-radius: var(--radius);
		background: var(--bg-elevated);
		font-size: var(--text-sm);
		font-weight: 500;
		box-shadow: var(--shadow-sm);
	}
	.verdict-row.caught {
		outline: 1px solid color-mix(in srgb, var(--bad) 35%, transparent);
	}
	.small {
		font-size: var(--text-xs);
		margin: var(--s-2) 0 0;
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
		border-radius: var(--radius-xl);
	}
	.cta h2 {
		font-size: var(--text-2xl);
		letter-spacing: -0.035em;
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
		.safety-grid {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}
	}
	@media (max-width: 760px) {
		.proof {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}
		.steps,
		.uses,
		.safety-grid {
			grid-template-columns: 1fr;
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
		.hero-card {
			left: var(--s-3);
			right: var(--s-3);
			bottom: var(--s-3);
			padding: var(--s-3) var(--s-4);
		}
	}
</style>
