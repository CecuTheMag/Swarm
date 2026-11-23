<script lang="ts">
	import { untrack } from 'svelte';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { LiveDashboard } from '#lib/dashboard/live.svelte.ts';
	import { sortedJobs } from '#lib/dashboard/state.ts';
	import { api, failureText } from '#lib/api.ts';
	import { can, useNetwork } from '#lib/console/networks.svelte.ts';
	import ConfirmButton from '#lib/ui/ConfirmButton.svelte';
	import Icon from '#lib/ui/Icon.svelte';
	import type { JobStatus } from '#lib/protocol/JobStatus.ts';
	import type { JobSummary } from '#lib/protocol/JobSummary.ts';
	import type { PiResult } from '#lib/protocol/PiResult.ts';
	import type { PtResult } from '#lib/protocol/PtResult.ts';
	import type { NnResult } from '#lib/protocol/NnResult.ts';
	import type { ProgramJobInfo } from '#lib/protocol/ProgramJobInfo.ts';
	import type { ProgramSummary } from '#lib/protocol/ProgramSummary.ts';
	import type { TtResult } from '#lib/protocol/TtResult.ts';
	import type { TtSchool } from '#lib/protocol/TtSchool.ts';
	import { downloadOutputs, encodeArgs, parseInputs } from '#lib/console/programs.ts';
	import { parsePrompts } from '#lib/console/ai.ts';
	import { aiCatalogText } from '#lib/templates.ts';
	import type { AiCatalog } from '#lib/protocol/AiCatalog.ts';

	const network = useNetwork();
	const id = $derived(page.params.id!);
	const admin = $derived(network.view ? can(network.view.role, 'admin') : false);

	let live = $state<LiveDashboard | null>(null);
	$effect(() => {
		const l = new LiveDashboard(undefined, `/ws/networks/${id}/live`);
		l.start();
		live = l;
		return () => l.stop();
	});
	const jobs = $derived(live ? sortedJobs(live.state) : []);

	// What to compute, in plain terms; the technical fields stay one click away.
	let template = $state<'pt' | 'nn' | 'pi' | 'program' | 'tt' | 'ai'>('pt');
	let quality = $state<'fast' | 'normal' | 'high'>('normal');
	const QUALITY = {
		fast: { width: 960, height: 540, spp: 64, passes: 4 },
		normal: { width: 1280, height: 720, spp: 128, passes: 8 },
		high: { width: 1920, height: 1080, spp: 128, passes: 16 }
	};
	let advanced = $state(false);
	let width = $state(1280);
	let height = $state(720);
	let tile = $state(64);
	let spp = $state(128);
	let passes = $state(8);
	let units = $state(64);
	let samples = $state(16_777_216);
	let seed = $state(42);
	$effect(() => {
		({ width, height, spp, passes } = QUALITY[quality]);
	});

	// A neural network sweep (spec 0011): which settings train best. Each preset is
	// a grid of settings; every device trains one combination. Real handwriting learns
	// from more, smaller steps; every setting stays within a unit's work limit
	// (kernels::nn::MAX_WORK).
	const SWEEPS = {
		mnist: {
			quick: {
				hidden: [64, 128, 192],
				layers: [1],
				lr: [0.2, 0.4],
				batch: [16],
				steps: 2000,
				seeds: 1
			},
			normal: {
				hidden: [64, 128, 192],
				layers: [1, 2],
				lr: [0.1, 0.2, 0.4],
				batch: [16],
				steps: 3000,
				seeds: 2
			},
			deep: {
				hidden: [32, 64, 96, 128, 192, 256],
				layers: [1, 2],
				lr: [0.05, 0.1, 0.2, 0.3, 0.45],
				batch: [8, 16],
				steps: 2500,
				seeds: 2
			}
		},
		glyphs: {
			quick: {
				hidden: [32, 64, 128],
				layers: [1],
				lr: [0.05, 0.2],
				batch: [32],
				steps: 300,
				seeds: 1
			},
			normal: {
				hidden: [32, 64, 128],
				layers: [1, 2],
				lr: [0.05, 0.1, 0.2, 0.4],
				batch: [32],
				steps: 600,
				seeds: 2
			},
			deep: {
				hidden: [16, 32, 64, 128, 256],
				layers: [1, 2],
				lr: [0.02, 0.05, 0.1, 0.2, 0.4],
				batch: [16, 32],
				steps: 600,
				seeds: 2
			}
		}
	};
	let dataset = $state<keyof typeof SWEEPS>('mnist');
	let sweep = $state<keyof typeof SWEEPS.mnist>('normal');
	let nnHidden = $state('');
	let nnLayers = $state('');
	let nnLr = $state('');
	let nnBatch = $state('');
	let nnSteps = $state(600);
	let nnSeeds = $state(2);
	$effect(() => {
		const s = SWEEPS[dataset][sweep];
		nnHidden = s.hidden.join(', ');
		nnLayers = s.layers.join(', ');
		nnLr = s.lr.join(', ');
		nnBatch = s.batch.join(', ');
		nnSteps = s.steps;
		nnSeeds = s.seeds;
	});
	/** "32, 64 128" → [32, 64, 128]; values are separated by commas or spaces. */
	const list = (text: string) =>
		text
			.split(/[\s,;]+/)
			.filter((v) => v !== '')
			.map(Number)
			.filter((v) => Number.isFinite(v));
	const configs = $derived(
		list(nnHidden).length * list(nnLayers).length * list(nnLr).length * list(nnBatch).length
	);

	// The network's own program (spec 0012).
	let programs = $state<ProgramSummary[]>([]);
	$effect(() => {
		api<ProgramSummary[]>(`/api/networks/${id}/programs`).then(
			(p) => {
				programs = p;
				// The choice shown is the one used: the newest program until another is picked.
				if (!p.some((x) => x.id === untrack(() => programId))) programId = p[0]?.id ?? '';
			},
			() => (programs = [])
		);
	});
	let programId = $state('');
	const program = $derived(programs.find((p) => p.id === programId) ?? programs[0]);
	let inputMode = $state<'count' | 'file'>('count');
	let count = $state(100);
	let args = $state('');
	let argsHex = $state(false);
	let inputsFile = $state<File | null>(null);
	let quorum = $state(2);
	let onPublic = $state(false);
	const canGoPublic = $derived(program?.review === 'approved' && !network.view?.public);

	/** The body of a program job, from the form. */
	async function programJob() {
		if (!program) throw new Error('Първо качете програма в раздел „Програми“.');
		const params =
			inputMode === 'count'
				? { program: program.id, count, args: encodeArgs(args, argsHex) }
				: { program: program.id, inputs: parseInputs((await inputsFile?.text()) ?? '') };
		const publicJob = canGoPublic && onPublic;
		return {
			template: 'program',
			params,
			...(publicJob ? { public: true } : network.view?.public ? {} : { quorum })
		};
	}

	// A school timetable (spec 0013): many searches, the best one wins.
	let ttSource = $state<'example' | 'file'>('example');
	let ttClasses = $state(20);
	let ttFile = $state<File | null>(null);
	let ttEffort = $state<'quick' | 'normal' | 'deep'>('normal');
	let ttSearches = $state(32);
	const TT_ITERATIONS = { quick: 500_000, normal: 2_000_000, deep: 8_000_000 };

	/** The body of a timetable job, from the form. */
	async function timetableJob() {
		const school: TtSchool =
			ttSource === 'example'
				? await api<TtSchool>(`/api/timetables/example?classes=${ttClasses}&seed=${seed}`)
				: (JSON.parse((await ttFile?.text()) ?? 'null') as TtSchool);
		if (!school) throw new Error('Изберете файл с данните на училището.');
		return {
			template: 'tt',
			params: { school, searches: ttSearches, iterations: TT_ITERATIONS[ttEffort], seed },
			...(network.view?.public ? {} : { quorum })
		};
	}

	// Questions for a language model (spec 0017): every capable device runs the whole
	// model and answers some of them. Test models are listed only when the address asks.
	const aiModels = $derived(
		(JSON.parse(aiCatalogText) as AiCatalog).models.filter(
			(m) => !m.test || page.url.searchParams.has('test-models')
		)
	);
	let aiModel = $state('');
	const aiChosen = $derived(aiModels.find((m) => m.id === aiModel) ?? aiModels[0]);
	// The choice shown is the one used: the first model until another is picked.
	$effect(() => {
		if (!aiModels.some((m) => m.id === untrack(() => aiModel))) aiModel = aiModels[0]?.id ?? '';
	});
	let aiSource = $state<'text' | 'file'>('text');
	let aiText = $state('');
	let aiFile = $state<File | null>(null);
	let aiSystem = $state('Отговаряй кратко и ясно.');
	let aiTokens = $state(128);
	const aiSize = (m: (typeof aiModels)[number]) =>
		Math.round(m.files.reduce((sum, f) => sum + f.size, 0) / 1e6);

	/** The body of a language-model job, from the form. */
	async function aiJob() {
		const prompts = parsePrompts(aiSource === 'text' ? aiText : ((await aiFile?.text()) ?? ''));
		if (prompts.length === 0) throw new Error('Напишете поне един въпрос.');
		return {
			template: 'ai',
			params: {
				model: aiChosen.id,
				prompts,
				...(aiSystem.trim() ? { system: aiSystem.trim() } : {}),
				max_new_tokens: aiTokens
			},
			...(network.view?.public ? {} : { quorum })
		};
	}

	let error = $state<string | null>(null);
	let busy = $state(false);

	async function start(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		error = null;
		try {
			await api<JobSummary>(`/api/networks/${id}/jobs`, {
				body:
					template === 'program'
						? await programJob()
						: template === 'ai'
							? await aiJob()
							: template === 'tt'
								? await timetableJob()
								: template === 'pi'
									? { template, params: { units, samples_per_unit: samples, seed } }
									: template === 'nn'
										? {
												template,
												params: {
													dataset,
													steps: nnSteps,
													hidden: list(nnHidden),
													layers: list(nnLayers),
													learning_rate: list(nnLr),
													batch: list(nnBatch),
													seeds: nnSeeds,
													seed
												}
											}
										: { template, params: { width, height, tile, spp, passes, seed } }
			});
		} catch (e) {
			error = failureText(e);
		} finally {
			busy = false;
		}
	}

	async function stop(job: string) {
		error = null;
		try {
			await api(`/api/networks/${id}/jobs/${job}/cancel`, { body: {} });
		} catch (e) {
			error = failureText(e);
		}
	}

	const statusText: Record<JobStatus, string> = {
		queued: 'чака',
		running: 'изчислява се',
		done: 'готова',
		failed: 'неуспешна'
	};
	const piResult = (r: unknown) => r as PiResult;
	const ptResult = (r: unknown) => r as PtResult;
	const nnResult = (r: unknown) => r as NnResult;
	const kindName: Record<string, string> = {
		pt: 'Изображение',
		nn: 'Невронна мрежа',
		pi: 'Числото π',
		tt: 'Разписание',
		ai: 'Текст с ИИ'
	};
	const kindIcon = {
		pt: 'image',
		nn: 'network',
		pi: 'atom',
		program: 'code',
		tt: 'school',
		ai: 'chip'
	} as const;
	const ttResult = (r: unknown) => r as TtResult;
	/** A program job shows its program's name. */
	const jobName = (job: JobSummary) =>
		job.template === 'program'
			? (job.params as unknown as ProgramJobInfo).name
			: (kindName[job.template] ?? job.template);

	async function download(job: string) {
		error = null;
		try {
			await downloadOutputs(id, job);
		} catch (e) {
			error = failureText(e);
		}
	}
</script>

<svelte:head>
	<title>Задачи · {network.view?.name} · Рояк</title>
</svelte:head>

{#if admin}
	<section class="card new">
		<h2>Нова задача</h2>
		<form onsubmit={start}>
			<div class="segmented kinds stack-sm" role="group" aria-label="Какво да се изчисли">
				<button type="button" aria-pressed={template === 'pt'} onclick={() => (template = 'pt')}>
					<Icon name="image" size={16} /> Изображение
				</button>
				<button
					type="button"
					aria-pressed={template === 'nn'}
					onclick={() => (template = 'nn')}
					data-testid="kind-nn"
				>
					<Icon name="network" size={16} /> Невронна мрежа
				</button>
				<button type="button" aria-pressed={template === 'pi'} onclick={() => (template = 'pi')}>
					<Icon name="atom" size={16} /> Числото π
				</button>
				<button
					type="button"
					aria-pressed={template === 'tt'}
					onclick={() => (template = 'tt')}
					data-testid="kind-tt"
				>
					<Icon name="school" size={16} /> Разписание
				</button>
				<button
					type="button"
					aria-pressed={template === 'ai'}
					onclick={() => (template = 'ai')}
					data-testid="kind-ai"
				>
					<Icon name="chip" size={16} /> Текст с ИИ
				</button>
				<button
					type="button"
					aria-pressed={template === 'program'}
					onclick={() => (template = 'program')}
					data-testid="kind-program"
				>
					<Icon name="code" size={16} /> Своя програма
				</button>
			</div>

			{#if template === 'pt'}
				<p class="muted">
					Реалистично изображение чрез проследяване на лъчи. По-високото качество отнема повече
					време.
				</p>
				<div class="segmented" role="group" aria-label="Качество">
					<button type="button" aria-pressed={quality === 'fast'} onclick={() => (quality = 'fast')}
						>Бързо</button
					>
					<button
						type="button"
						aria-pressed={quality === 'normal'}
						onclick={() => (quality = 'normal')}>Нормално</button
					>
					<button type="button" aria-pressed={quality === 'high'} onclick={() => (quality = 'high')}
						>Високо</button
					>
				</div>
			{:else if template === 'nn'}
				<p class="muted">
					Търси с кои настройки малка невронна мрежа се учи най-добре да разпознава ръкописни цифри.
					Всяко устройство обучава една настройка, а резултатите се подреждат в класация.
				</p>
				<div class="segmented stack-sm" role="group" aria-label="Цифри за обучение">
					<button
						type="button"
						aria-pressed={dataset === 'mnist'}
						onclick={() => (dataset = 'mnist')}
						data-testid="dataset-mnist">Истински почерк (MNIST)</button
					>
					<button
						type="button"
						aria-pressed={dataset === 'glyphs'}
						onclick={() => (dataset = 'glyphs')}
						data-testid="dataset-glyphs">Рисувани от компютъра</button
					>
				</div>
				<p class="muted small">
					{dataset === 'mnist'
						? 'Цифри, написани на ръка от 500 души: 10 000 за учене и 2 000 за проверка (2,4 MB, изтегля се веднъж на устройство).'
						: 'Цифри, които всяко устройство рисува само: нищо не се изтегля.'}
				</p>
				<div class="segmented stack-sm" role="group" aria-label="Обхват на търсенето">
					<button type="button" aria-pressed={sweep === 'quick'} onclick={() => (sweep = 'quick')}
						>Бързо сравнение</button
					>
					<button type="button" aria-pressed={sweep === 'normal'} onclick={() => (sweep = 'normal')}
						>Нормално</button
					>
					<button type="button" aria-pressed={sweep === 'deep'} onclick={() => (sweep = 'deep')}
						>Задълбочено</button
					>
				</div>
				<p class="muted small" data-testid="sweep-size">
					{configs} настройки × {nnSeeds}
					{nnSeeds === 1 ? 'обучение' : 'обучения'} = {configs * nnSeeds} части
				</p>
			{:else if template === 'tt'}
				<p class="muted">
					Седмичното разписание на училище: без застъпвания и прозорци за учениците, с възможно
					най-малко прозорци за учителите. Всяко устройство търси от различно начало; печели
					най-доброто, а сървърът проверява оценката на всяко.
				</p>
				<div class="segmented stack-sm" role="group" aria-label="Данни">
					<button
						type="button"
						aria-pressed={ttSource === 'example'}
						onclick={() => (ttSource = 'example')}>Примерно училище</button
					>
					<button
						type="button"
						aria-pressed={ttSource === 'file'}
						onclick={() => (ttSource = 'file')}
						data-testid="tt-file-mode">Данни от файл</button
					>
				</div>
				{#if ttSource === 'example'}
					<label
						>Паралелки
						<select bind:value={ttClasses} data-testid="tt-classes">
							{#each [5, 10, 20, 30, 40, 60] as n (n)}
								<option value={n}>{n} (гимназия, 8.–12. клас)</option>
							{/each}
						</select></label
					>
				{:else}
					<label
						>Файл с училището (JSON)
						<input
							type="file"
							accept=".json,application/json"
							required
							data-testid="tt-file"
							onchange={(e) => (ttFile = e.currentTarget.files?.[0] ?? null)}
						/></label
					>
				{/if}
				<p class="muted">
					Своето училище въведете или внесете от таблица в раздел
					<a href={resolve('/app/n/[id]/schools', { id })}>„Училища“</a> и търсете разписание оттам.
				</p>
				<div class="segmented stack-sm" role="group" aria-label="Колко да се търси">
					<button
						type="button"
						aria-pressed={ttEffort === 'quick'}
						onclick={() => (ttEffort = 'quick')}
						data-testid="tt-quick">Бързо</button
					>
					<button
						type="button"
						aria-pressed={ttEffort === 'normal'}
						onclick={() => (ttEffort = 'normal')}>Нормално</button
					>
					<button
						type="button"
						aria-pressed={ttEffort === 'deep'}
						onclick={() => (ttEffort = 'deep')}>Задълбочено</button
					>
				</div>
				<div class="fields">
					<label
						>Търсения <input
							type="number"
							min="1"
							max="500"
							bind:value={ttSearches}
							data-testid="tt-searches"
						/></label
					>
					{#if !network.view?.public}
						<label
							>Проверка
							<select bind:value={quorum} data-testid="tt-quorum">
								<option value={1}>1 резултат (вярвам на устройствата)</option>
								<option value={2}>2 съвпадащи резултата</option>
								<option value={3}>3 съвпадащи резултата</option>
							</select></label
						>
					{/if}
				</div>
			{:else if template === 'ai'}
				<p class="muted">
					Езиков модел отговаря на много въпроси наведнъж. Всяко устройство с видеокарта пуска целия
					модел в браузъра и отговаря на част от тях: нищо не излиза извън устройствата и сървъра.
				</p>
				<div class="fields">
					<label
						>Модел
						<select bind:value={aiModel} data-testid="ai-model">
							{#each aiModels as m (m.id)}
								<option value={m.id}
									>{m.name} ({aiSize(m)} MB{m.test ? ', само за тестове' : ''})</option
								>
							{/each}
						</select></label
					>
					<label
						>Дължина на отговора
						<select bind:value={aiTokens} data-testid="ai-tokens">
							<option value={32}>кратък (до 32 токена)</option>
							<option value={128}>нормален (до 128 токена)</option>
							<option value={256}>дълъг (до 256 токена)</option>
							<option value={512}>много дълъг (до 512 токена)</option>
						</select></label
					>
					{#if !network.view?.public}
						<label
							>Проверка
							<select bind:value={quorum} data-testid="ai-quorum">
								<option value={1}>1 отговор (вярвам на устройствата)</option>
								<option value={2}>2 еднакви отговора</option>
								<option value={3}>3 еднакви отговора</option>
							</select></label
						>
					{/if}
				</div>
				<p class="muted small">
					Устройствата изтеглят модела веднъж ({aiChosen ? aiSize(aiChosen) : 0} MB) и го пазят. Лиценз:
					{aiChosen?.licence}.
				</p>
				<label
					>Указания към модела
					<input bind:value={aiSystem} maxlength="2000" data-testid="ai-system" /></label
				>
				<div class="segmented stack-sm" role="group" aria-label="Въпроси">
					<button
						type="button"
						aria-pressed={aiSource === 'text'}
						onclick={() => (aiSource = 'text')}>Написани тук</button
					>
					<button
						type="button"
						aria-pressed={aiSource === 'file'}
						onclick={() => (aiSource = 'file')}
						data-testid="ai-file-mode">От файл</button
					>
				</div>
				{#if aiSource === 'text'}
					<label
						>Въпроси, по един на ред
						<textarea
							rows="6"
							bind:value={aiText}
							required
							placeholder="Какво е фотосинтеза?&#10;Назови три реки в България."
							data-testid="ai-prompts"></textarea></label
					>
				{:else}
					<label
						>Файл с въпроси (по един на ред или JSON Lines с поле "prompt")
						<input
							type="file"
							accept=".txt,.jsonl,.csv,text/plain"
							required
							data-testid="ai-file"
							onchange={(e) => (aiFile = e.currentTarget.files?.[0] ?? null)}
						/></label
					>
				{/if}
			{:else if template === 'program'}
				{#if programs.length === 0}
					<p class="muted">
						Първо качете програма в раздел <a href={resolve('/app/n/[id]/programs', { id })}
							>„Програми“</a
						>.
					</p>
				{:else}
					<p class="muted">
						Вашата програма изчислява всяка част на отделно устройство; резултатите се теглят като
						файл.
					</p>
					<div class="fields">
						<label
							>Програма
							<select bind:value={programId} data-testid="job-program">
								{#each programs as p (p.id)}
									<option value={p.id}>{p.name}</option>
								{/each}
							</select></label
						>
						{#if !(canGoPublic && onPublic) && !network.view?.public}
							<label
								>Проверка
								<select bind:value={quorum} data-testid="job-quorum">
									<option value={1}>1 резултат (вярвам на устройствата)</option>
									<option value={2}>2 съвпадащи резултата</option>
									<option value={3}>3 съвпадащи резултата</option>
								</select></label
							>
						{/if}
					</div>
					<div class="segmented stack-sm" role="group" aria-label="Входове">
						<button
							type="button"
							aria-pressed={inputMode === 'count'}
							onclick={() => (inputMode = 'count')}>Брой части</button
						>
						<button
							type="button"
							aria-pressed={inputMode === 'file'}
							onclick={() => (inputMode = 'file')}
							data-testid="inputs-file-mode">Входове от файл</button
						>
					</div>
					{#if inputMode === 'count'}
						<p class="muted small">
							Част <em>i</em> получава числото <em>i</em> (4 байта) и общите аргументи.
						</p>
						<div class="fields">
							<label
								>Части <input
									type="number"
									min="1"
									max="20000"
									bind:value={count}
									data-testid="job-count"
								/></label
							>
							<label
								>Общи аргументи <input
									bind:value={args}
									placeholder={argsHex ? '40420f0000000000' : 'текст'}
								/></label
							>
							<label class="check"
								><input type="checkbox" bind:checked={argsHex} /> шестнадесетично</label
							>
						</div>
					{:else}
						<p class="muted small">
							По един вход на ред (текст) или JSON масив от низове в base64.
						</p>
						<label
							>Файл с входове
							<input
								type="file"
								required
								data-testid="job-inputs"
								onchange={(e) => (inputsFile = e.currentTarget.files?.[0] ?? null)}
							/></label
						>
					{/if}
					{#if canGoPublic}
						<label class="check"
							><input type="checkbox" bind:checked={onPublic} data-testid="job-public" />
							На общата мрежа: програмата е одобрена и може да се изчислява от всички доброволци</label
						>
					{/if}
				{/if}
			{:else}
				<p class="muted">Числото π по метода Монте Карло: точен, проверим резултат.</p>
			{/if}

			<details
				bind:open={advanced}
				hidden={template === 'program' || template === 'tt' || template === 'ai'}
			>
				<summary>Подробни настройки</summary>
				<div class="fields">
					{#if template === 'nn'}
						<label>Неврони в слой <input bind:value={nnHidden} inputmode="numeric" /></label>
						<label>Скрити слоеве <input bind:value={nnLayers} inputmode="numeric" /></label>
						<label>Скорост на учене <input bind:value={nnLr} inputmode="decimal" /></label>
						<label>Партида <input bind:value={nnBatch} inputmode="numeric" /></label>
						<label>Стъпки <input type="number" min="1" max="5000" bind:value={nnSteps} /></label>
						<label
							>Обучения на настройка <input
								type="number"
								min="1"
								max="5"
								bind:value={nnSeeds}
							/></label
						>
					{:else if template === 'pi'}
						<label>Единици <input type="number" min="1" max="10000" bind:value={units} /></label>
						<label
							>Проби на единица <input
								type="number"
								min="1"
								max="1073741824"
								bind:value={samples}
							/></label
						>
					{:else}
						<label>Ширина <input type="number" min="16" max="2048" bind:value={width} /></label>
						<label>Височина <input type="number" min="16" max="2048" bind:value={height} /></label>
						<label>Плочка <input type="number" min="8" max="128" bind:value={tile} /></label>
						<label
							>Проби на проход <input type="number" min="1" max="1024" bind:value={spp} /></label
						>
						<label>Проходи <input type="number" min="1" max="256" bind:value={passes} /></label>
					{/if}
					<label>Начално число <input type="number" min="0" bind:value={seed} /></label>
				</div>
			</details>
			{#if error}<p class="notice bad" role="alert">{error}</p>{/if}
			<button type="submit" class="lg" disabled={busy} data-testid="start-job">Пусни</button>
		</form>
	</section>
{/if}

<section class="card">
	<h2>Задачи</h2>
	{#if jobs.length === 0}
		<p class="muted">Още няма задачи{admin ? '. Пуснете първата отгоре.' : '.'}</p>
	{:else}
		<ul class="jobs">
			{#each jobs as job (job.id)}
				<li class="job" data-testid="job" data-job-id={job.id} data-status={job.status}>
					<div class="main">
						<span class="kind">
							<Icon name={kindIcon[job.template as keyof typeof kindIcon] ?? 'atom'} size={18} />
							{jobName(job)}
						</span>
						{#if job.owner_network_id}<span class="badge">на общата мрежа</span>{/if}
						<span
							class="badge"
							class:ok={job.status === 'done'}
							class:bad={job.status === 'failed'}
							class:live={job.status === 'running'}
						>
							{job.error === 'cancelled' ? 'спряна' : statusText[job.status]}
						</span>
					</div>
					<div class="prog">
						<div class="progress">
							<div style:width="{(100 * job.units_done) / job.units_total}%"></div>
						</div>
						<span class="muted count">{job.units_done}/{job.units_total}</span>
					</div>
					<div class="result">
						{#if job.template === 'pt'}
							<a href="{resolve('/app/n/[id]/live', { id })}?job={job.id}">На живо</a>
							{#if job.result}
								<span class="muted">{ptResult(job.result).width}×{ptResult(job.result).height}</span
								>
							{/if}
						{:else if job.template === 'nn'}
							<a
								href={resolve('/app/n/[id]/jobs/[job]', { id, job: job.id })}
								data-testid="sweep-link">Класация</a
							>
							{#if job.result && nnResult(job.result).rows.length > 0}
								<span class="muted"
									>най-добра: {(nnResult(job.result).rows[0].accuracy * 100).toFixed(1)} %</span
								>
							{/if}
						{:else if job.template === 'tt'}
							<a
								href={resolve('/app/n/[id]/jobs/[job]/timetable', { id, job: job.id })}
								data-testid="timetable-link">Разписание</a
							>
							{#if job.result}
								<span class="muted"
									>{ttResult(job.result).cost.hard === 0 ? 'валидно' : 'с нарушения'} · оценка {ttResult(
										job.result
									).cost.total}</span
								>
							{/if}
						{:else if job.template === 'ai'}
							<a
								href={resolve('/app/n/[id]/jobs/[job]/answers', { id, job: job.id })}
								data-testid="answers-link">Отговори</a
							>
						{:else if job.template === 'program'}
							{#if job.status === 'done'}
								<button class="secondary sm" data-testid="download" onclick={() => download(job.id)}
									>Изтегли резултатите</button
								>
							{/if}
						{:else if job.result}
							<span>π ≈ <strong>{piResult(job.result).pi.toFixed(6)}</strong></span>
						{/if}
						{#if job.error && job.error !== 'cancelled'}
							<span class="muted small">{job.error}</span>
						{/if}
					</div>
					{#if admin && (job.status === 'queued' || job.status === 'running')}
						<ConfirmButton
							label="Спри"
							confirm="Спри задачата?"
							onconfirm={() => stop(job.id)}
							testid="stop-job"
						/>
					{/if}
				</li>
			{/each}
		</ul>
	{/if}
</section>

<style>
	section + section {
		margin-top: var(--s-4);
	}
	h2 {
		font-size: var(--text-lg);
		margin: 0 0 var(--s-3);
	}
	.new form {
		display: grid;
		gap: var(--s-4);
	}
	.new p {
		margin: 0;
	}
	/* Six kinds do not fit one row with their icons: they wrap, as rounded rectangles. */
	.kinds {
		justify-self: stretch;
		grid-auto-flow: row;
		grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
		border-radius: 14px;
	}
	.kinds button {
		border-radius: 11px;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: 6px;
	}
	.kinds :global(svg) {
		flex-shrink: 0;
	}
	details summary {
		cursor: pointer;
		font-weight: 600;
		font-size: var(--text-sm);
	}
	.check {
		display: flex;
		gap: var(--s-2);
		align-items: center;
	}
	.check input {
		width: auto;
	}
	.fields {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
		gap: var(--s-3);
		margin-top: var(--s-3);
	}
	.new button[type='submit'] {
		justify-self: start;
	}
	.jobs {
		list-style: none;
		margin: 0;
		padding: 0;
	}
	.job {
		display: grid;
		grid-template-columns: minmax(150px, 1fr) minmax(140px, 1fr) minmax(0, 1.2fr) auto;
		gap: var(--s-4);
		align-items: center;
		padding: 14px 0;
		border-bottom: 1px solid var(--border);
		font-size: var(--text-sm);
	}
	.job:last-child {
		border-bottom: 0;
	}
	.main {
		display: flex;
		gap: var(--s-3);
		align-items: center;
		flex-wrap: wrap;
	}
	.kind {
		display: inline-flex;
		gap: 8px;
		align-items: center;
		font-weight: 600;
	}
	.prog {
		display: flex;
		gap: var(--s-3);
		align-items: center;
	}
	.prog .progress {
		flex: 1;
	}
	.count {
		font-size: var(--text-xs);
		font-variant-numeric: tabular-nums;
	}
	.result {
		display: grid;
		gap: 2px;
		min-width: 0;
		overflow-wrap: anywhere;
	}
	.small {
		font-size: var(--text-xs);
	}
	/* Phones: three long choices read better as a column than squeezed in a row. */
	@media (max-width: 480px) {
		.stack-sm {
			grid-auto-flow: row;
		}
	}
	@media (max-width: 800px) {
		.job {
			grid-template-columns: minmax(0, 1fr) auto;
			gap: var(--s-2) var(--s-3);
		}
		.prog,
		.result {
			grid-column: 1 / -1;
		}
	}

	/* Phones: what to compute is a rail of chips to swipe, not six stacked rows. */
	@media (max-width: 760px) {
		.kinds {
			display: flex;
			gap: 6px;
			margin: 0 calc(-1 * var(--s-5));
			padding: 0 var(--s-5);
			overflow-x: auto;
			scroll-snap-type: x proximity;
			scroll-padding-inline: var(--s-5);
			scrollbar-width: none;
			background: none;
			border-radius: 0;
		}
		.kinds::-webkit-scrollbar {
			display: none;
		}
		.kinds button {
			flex: none;
			scroll-snap-align: start;
			min-height: 44px;
			padding: 8px 16px;
			border-radius: var(--pill);
			white-space: nowrap;
			background: rgb(118 118 128 / 0.12);
		}
		.kinds button :global(svg) {
			display: inline;
		}
	}
</style>
