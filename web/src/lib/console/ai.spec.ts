import { describe, expect, it } from 'vitest';
import { answersCsv, answersText, parsePrompts } from './ai.ts';

describe('prompts', () => {
	it('reads one prompt per line and skips empty lines', () => {
		expect(parsePrompts('Какво е фотосинтеза?\r\n\nНазови три реки.\n')).toEqual([
			'Какво е фотосинтеза?',
			'Назови три реки.'
		]);
	});

	it('reads JSON Lines of strings or objects', () => {
		expect(parsePrompts('{"prompt": "a, b"}\n"c"\n{"prompt": "d", "id": 4}')).toEqual([
			'a, b',
			'c',
			'd'
		]);
	});

	it('says which JSON line is wrong', () => {
		expect(() => parsePrompts('{"prompt": "a"}\n{"text": "b"}')).toThrow(/Ред 2/);
		expect(() => parsePrompts('{"prompt": "a"}\n{oops')).toThrow(/Ред 2/);
	});

	it('keeps a line that only starts like JSON as text when others are not', () => {
		expect(parsePrompts('"quoted" title\nplain')).toEqual(['"quoted" title', 'plain']);
	});
});

describe('answers', () => {
	it('are a CSV in prompt order, quoted where needed', () => {
		const csv = answersCsv(['Кой, кога?', 'Защо'], ['Той каза "да"', null]);
		expect(csv.startsWith('﻿')).toBe(true);
		expect(csv.slice(1).split('\r\n')).toEqual([
			'№,Въпрос,Отговор',
			'1,"Кой, кога?","Той каза ""да"""',
			'2,Защо,',
			''
		]);
	});

	it('are plain text with a dash for an answer still missing', () => {
		expect(answersText(['a', 'b'], ['x', null])).toBe('1. a\nx\n\n2. b\n—\n');
	});
});
