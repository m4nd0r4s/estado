import fs from 'node:fs';
import path from 'node:path';
import { Marked, type Token, type Tokens } from 'marked';
import { highlight } from './highlight.ts';
import { basePath, githubUrl, repoRoot, toRepoPath } from './repo.ts';

export interface Heading {
	depth: number;
	text: string;
	id: string;
}

export interface RenderedMarkdown {
	html: string;
	headings: Heading[];
	/** The first paragraph as plain text, for the page description. */
	summary: string;
}

/** Maps a repository path (`docs/errors/E0003.md`) to its site route, if it has one. */
export type RouteLookup = (repoPath: string) => string | undefined;

type HighlightedCode = Tokens.Code & { highlighted?: string };

/** Raw HTML the docs may contain; anything else fails the build. */
const allowedHtml = /^<\/?(code|br|kbd|sub|sup)>$/i;

/** GitHub-style heading ids: lowercase, punctuation dropped, spaces to dashes. */
export function slugify(text: string): string {
	return text
		.toLowerCase()
		.replace(/[^\p{Letter}\p{Mark}\p{Number}\s_-]/gu, '')
		.trim()
		.replace(/\s/g, '-');
}

/** The text a reader sees, without Markdown syntax or HTML escapes. */
export function plainText(tokens: Token[] | undefined): string {
	return (tokens ?? [])
		.map((token) => {
			if ('tokens' in token && token.tokens) return plainText(token.tokens);
			if (token.type === 'codespan') return (token as Tokens.Codespan).text;
			return 'text' in token ? String(token.text) : '';
		})
		.join('')
		.replace(/&amp;/g, '&')
		.replace(/&lt;/g, '<')
		.replace(/&gt;/g, '>')
		.replace(/&quot;/g, '"')
		.replace(/&#39;/g, "'");
}

function escapeAttribute(value: string): string {
	return value.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;');
}

/**
 * Resolves a link written in `sourcePath` to a site route, a GitHub URL or
 * itself. A relative link to a file that does not exist fails the build.
 */
export function resolveLink(href: string, sourcePath: string, routeOf: RouteLookup): string {
	if (/^(javascript|data|vbscript):/i.test(href.trim())) {
		throw new Error(`${sourcePath}: unsafe link ${href}`);
	}
	if (/^[a-z][a-z0-9+.-]*:/i.test(href) || href.startsWith('#') || href.startsWith('//')) {
		return href;
	}
	const hashIndex = href.indexOf('#');
	const target = decodeURIComponent(hashIndex === -1 ? href : href.slice(0, hashIndex));
	const anchor = hashIndex === -1 ? '' : href.slice(hashIndex);
	const absolute = path.resolve(path.dirname(path.join(repoRoot, sourcePath)), target);
	const repoPath = toRepoPath(absolute);
	if (repoPath.startsWith('..')) return href;
	const route = routeOf(repoPath);
	if (route !== undefined) return `${basePath}${route}${anchor}`;
	if (!fs.existsSync(absolute)) throw new Error(`${sourcePath}: broken link ${href}`);
	return `${githubUrl(repoPath)}${anchor}`;
}

/** Renders one Markdown file of the repository. */
export async function renderMarkdown(
	source: string,
	sourcePath: string,
	routeOf: RouteLookup
): Promise<RenderedMarkdown> {
	const headings: Heading[] = [];
	const usedIds = new Map<string, number>();
	let summary = '';

	const marked = new Marked({
		async: true,
		gfm: true,
		async walkTokens(token) {
			if (token.type === 'code') {
				const code = token as HighlightedCode;
				code.highlighted = await highlight(code.text, code.lang);
			} else if (token.type === 'html' && !allowedHtml.test(token.raw.trim())) {
				throw new Error(`${sourcePath}: raw HTML is not allowed in the docs: ${token.raw.trim()}`);
			} else if (token.type === 'paragraph' && summary === '') {
				summary = plainText((token as Tokens.Paragraph).tokens)
					.replace(/\s+/g, ' ')
					.trim();
			}
		},
		renderer: {
			code(token) {
				return (token as HighlightedCode).highlighted ?? false;
			},
			heading({ tokens, depth }) {
				const text = plainText(tokens);
				const base = slugify(text) || 'section';
				const seen = usedIds.get(base) ?? 0;
				usedIds.set(base, seen + 1);
				const id = seen === 0 ? base : `${base}-${seen}`;
				const inner = this.parser.parseInline(tokens);
				if (depth > 1) headings.push({ depth, text, id });
				if (depth === 1) return `<h1 id="${id}">${inner}</h1>\n`;
				return `<h${depth} id="${id}"><a class="anchor" href="#${id}" aria-hidden="true" tabindex="-1">#</a>${inner}</h${depth}>\n`;
			},
			link({ href, title, tokens }) {
				const resolved = resolveLink(href, sourcePath, routeOf);
				const external = /^https?:/.test(resolved);
				const titleAttribute = title ? ` title="${escapeAttribute(title)}"` : '';
				const rel = external ? ' rel="noopener"' : '';
				return `<a href="${escapeAttribute(resolved)}"${titleAttribute}${rel}>${this.parser.parseInline(tokens)}</a>`;
			}
		}
	});

	const html = (await marked.parse(source))
		.replace(
			/<table>/g,
			'<div class="table-scroll" role="region" aria-label="Table" tabindex="0"><table>'
		)
		.replace(/<\/table>/g, '</table></div>');
	return { html, headings, summary };
}
