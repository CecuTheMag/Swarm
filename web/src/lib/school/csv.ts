// CSV as spreadsheets save it (RFC 4180, and what Excel and LibreOffice write in
// Bulgarian locales): `;` or `,` between fields, quotes around fields that hold either,
// doubled quotes inside them, CRLF or LF, and a byte order mark in front.

/** One record and the line of the file it starts on, from 1. */
export interface CsvRow {
	line: number;
	fields: string[];
}

/** The separator of `text`: whichever of `;` and `,` its first line has more of. */
export function separatorOf(text: string): ';' | ',' {
	let semicolons = 0;
	let commas = 0;
	let quoted = false;
	for (const ch of text) {
		if (ch === '"') quoted = !quoted;
		else if (!quoted && (ch === '\n' || ch === '\r')) break;
		else if (!quoted && ch === ';') semicolons++;
		else if (!quoted && ch === ',') commas++;
	}
	return semicolons >= commas && semicolons > 0 ? ';' : ',';
}

/**
 * Every record of `text`, blank lines left out. Throws on a quote left open, with the
 * line it opened on.
 */
export function parseCsv(text: string, separator = separatorOf(text)): CsvRow[] {
	if (text.startsWith('﻿')) text = text.slice(1);
	const rows: CsvRow[] = [];
	let fields: string[] = [];
	let field = '';
	let quoted = false;
	let line = 1;
	let start = 1;
	let quoteLine = 1;
	const endRecord = () => {
		fields.push(field);
		if (fields.length > 1 || fields[0].trim() !== '') rows.push({ line: start, fields });
		fields = [];
		field = '';
	};
	for (let i = 0; i < text.length; i++) {
		const ch = text[i];
		if (quoted) {
			if (ch === '"' && text[i + 1] === '"') {
				field += '"';
				i++;
			} else if (ch === '"') {
				quoted = false;
			} else {
				if (ch === '\n') line++;
				field += ch;
			}
		} else if (ch === '"') {
			quoted = true;
			quoteLine = line;
		} else if (ch === separator) {
			fields.push(field);
			field = '';
		} else if (ch === '\n' || ch === '\r') {
			if (ch === '\r' && text[i + 1] === '\n') i++;
			endRecord();
			line++;
			start = line;
		} else {
			field += ch;
		}
	}
	if (quoted) throw new Error(`Ред ${quoteLine}: кавичките не са затворени.`);
	if (field !== '' || fields.length > 0) endRecord();
	return rows;
}

const needsQuotes = (v: string, separator: string) =>
	v.includes(separator) || /["\r\n]/.test(v) || v !== v.trim();

/** Rows as CSV that `parseCsv` reads back unchanged. */
export function writeCsv(rows: string[][], separator: ';' | ',' = ';'): string {
	return rows
		.map((r) =>
			r.map((v) => (needsQuotes(v, separator) ? `"${v.replaceAll('"', '""')}"` : v)).join(separator)
		)
		.join('\r\n')
		.concat('\r\n');
}
