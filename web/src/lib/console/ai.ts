// Language-model jobs in the console (spec 0017): prompts from a file or the form, and
// the answers saved as a table.

/**
 * Prompts from text: JSON Lines (each line a string, or an object with a `prompt`), or
 * else one prompt per line. Empty lines are skipped.
 */
export function parsePrompts(text: string): string[] {
	const lines = text
		.split(/\r?\n/)
		.map((l) => l.trim())
		.filter((l) => l !== '');
	const looksJson = lines.length > 0 && lines.every((l) => l.startsWith('{') || l.startsWith('"'));
	if (!looksJson) return lines;
	return lines.map((line, i) => {
		let value: unknown;
		try {
			value = JSON.parse(line);
		} catch {
			throw new Error(`Ред ${i + 1} не е валиден JSON.`);
		}
		if (typeof value === 'string') return value;
		const prompt = (value as { prompt?: unknown } | null)?.prompt;
		if (typeof prompt !== 'string') {
			throw new Error(`Ред ${i + 1} няма поле "prompt" с текст.`);
		}
		return prompt;
	});
}

const cell = (v: string) => (/[",\r\n]/.test(v) ? `"${v.replaceAll('"', '""')}"` : v);

/**
 * The answers as CSV, in prompt order: number, prompt, answer. A byte-order mark first,
 * so spreadsheet programs read the Cyrillic correctly.
 */
export function answersCsv(prompts: string[], answers: (string | null)[]): string {
	const rows = [['№', 'Въпрос', 'Отговор']];
	prompts.forEach((p, i) => rows.push([String(i + 1), p, answers[i] ?? '']));
	return '﻿' + rows.map((r) => r.map(cell).join(',')).join('\r\n') + '\r\n';
}

/** The answers as plain text: each prompt, then its answer, then a blank line. */
export function answersText(prompts: string[], answers: (string | null)[]): string {
	return prompts.map((p, i) => `${i + 1}. ${p}\n${answers[i] ?? '—'}\n`).join('\n');
}

export function saveFile(name: string, type: string, content: string) {
	const a = document.createElement('a');
	a.href = URL.createObjectURL(new Blob([content], { type }));
	a.download = name;
	a.click();
	URL.revokeObjectURL(a.href);
}
