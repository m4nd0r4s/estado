import { createHighlighter, type Highlighter } from 'shiki';
import { inkTheme } from './theme.ts';

const languages = ['rust', 'toml', 'bash', 'json', 'diff'] as const;
const aliases: Record<string, string> = { rs: 'rust', sh: 'bash', shell: 'bash', console: 'bash' };

let highlighter: Promise<Highlighter> | undefined;

function loadHighlighter(): Promise<Highlighter> {
	highlighter ??= createHighlighter({ themes: [inkTheme], langs: [...languages] });
	return highlighter;
}

/**
 * The language of a fence info string: `rust,ignore` is Rust, rustdoc's
 * attributes are dropped, unknown languages render as plain text.
 */
export function fenceLanguage(info: string | undefined): string {
	const name = (info ?? '').split(/[,\s]/)[0].toLowerCase();
	const resolved = aliases[name] ?? name;
	return (languages as readonly string[]).includes(resolved) ? resolved : 'text';
}

/**
 * Drops rustdoc's hidden lines (`# use ...;`), which only exist for doctests,
 * and unescapes `##` to a literal `#`, as rustdoc does.
 */
export function withoutHiddenLines(code: string): string {
	return code
		.split('\n')
		.filter((line) => !/^\s*#(\s|$)/.test(line))
		.map((line) => line.replace(/^(\s*)##/, '$1#'))
		.join('\n');
}

export async function highlight(code: string, info: string | undefined): Promise<string> {
	const language = fenceLanguage(info);
	const source = language === 'rust' ? withoutHiddenLines(code) : code;
	return (await loadHighlighter()).codeToHtml(source.replace(/\n+$/, ''), {
		lang: language,
		theme: 'estado-ink'
	});
}
