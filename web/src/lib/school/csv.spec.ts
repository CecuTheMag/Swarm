import { describe, expect, it } from 'vitest';
import { parseCsv, separatorOf, writeCsv } from './csv.ts';

describe('CSV', () => {
	it('reads what a spreadsheet saves in a Bulgarian locale', () => {
		const text =
			'﻿Паралелка;Предмет;Учител\r\n' +
			'8А;"Български език; литература";Петрова\r\n' +
			'\r\n' +
			'8Б;"Час на ""класа""";"Иванов, Г."\r\n';
		expect(parseCsv(text)).toEqual([
			{ line: 1, fields: ['Паралелка', 'Предмет', 'Учител'] },
			{ line: 2, fields: ['8А', 'Български език; литература', 'Петрова'] },
			{ line: 4, fields: ['8Б', 'Час на "класа"', 'Иванов, Г.'] }
		]);
	});

	it('picks the separator from the first line', () => {
		expect(separatorOf('a;b;c\n1,2;3')).toBe(';');
		expect(separatorOf('a,b,c\n1;2;3')).toBe(',');
		expect(separatorOf('"a;b",c,d')).toBe(',');
		expect(separatorOf('one column')).toBe(',');
	});

	it('keeps line breaks inside quotes and counts lines by where records start', () => {
		const rows = parseCsv('a,"two\nlines"\nb,c\n', ',');
		expect(rows).toEqual([
			{ line: 1, fields: ['a', 'two\nlines'] },
			{ line: 3, fields: ['b', 'c'] }
		]);
		// No newline at the end, and an empty last field.
		expect(parseCsv('x,y,', ',')).toEqual([{ line: 1, fields: ['x', 'y', ''] }]);
		expect(parseCsv('', ',')).toEqual([]);
	});

	it('says where a quote was left open', () => {
		expect(() => parseCsv('a,b\nc,"d\ne,f', ',')).toThrow('Ред 2');
	});

	it('reads back what it writes, whatever the fields hold', () => {
		const samples = ['', 'x', 'a;b', 'a,b', 'say "hi"', 'two\nlines', ' padded ', '\r', 'Ж'];
		for (const separator of [';', ','] as const) {
			for (const a of samples) {
				for (const b of samples) {
					const rows = [
						[a, b],
						['end', 'row']
					];
					const back = parseCsv(writeCsv(rows, separator), separator).map((r) => r.fields);
					expect(back).toEqual(rows);
				}
			}
		}
	});
});
