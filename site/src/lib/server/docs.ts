import fs from 'node:fs';
import path from 'node:path';
import { renderMarkdown, type Heading } from './markdown.ts';
import { repoRoot } from './repo.ts';

import type { NavSection } from '../types.ts';

export type { NavSection };
export type SectionId = NavSection['id'];

export interface DocEntry {
	/** Route under `/docs`, without a leading slash (`errors/E0003`). */
	slug: string;
	/** Path from the repository root (`docs/errors/E0003.md`). */
	sourcePath: string;
	section: SectionId;
	title: string;
	/** Short label for the sidebar. */
	label: string;
	/** Leading code shown in the sidebar (`E0003`, `0011`), if any. */
	code?: string;
}

export interface DocPage {
	entry: DocEntry;
	/** Title for the browser tab, unique across sections (`Handlers · Reference`). */
	pageTitle: string;
	description: string;
	html: string;
	headings: Heading[];
	previous?: Pick<DocEntry, 'slug' | 'title'>;
	next?: Pick<DocEntry, 'slug' | 'title'>;
}

const sectionTitles: Record<SectionId, string> = {
	guide: 'Guide',
	reference: 'Reference',
	errors: 'Error index'
};

const docsRoot = path.join(repoRoot, 'docs');

function markdownFiles(folder: string): string[] {
	const absolute = path.join(docsRoot, folder);
	if (!fs.existsSync(absolute)) return [];
	return fs
		.readdirSync(absolute)
		.filter((name) => name.endsWith('.md'))
		.sort((left, right) => {
			if (left === 'README.md') return -1;
			if (right === 'README.md') return 1;
			return left.localeCompare(right, 'en');
		});
}

/** The first `# ` heading, without Markdown code ticks. */
function titleOf(sourcePath: string): string {
	const text = fs.readFileSync(path.join(repoRoot, sourcePath), 'utf8');
	const heading = text.match(/^#\s+(.+)$/m)?.[1] ?? path.basename(sourcePath, '.md');
	return heading.replace(/`/g, '').trim();
}

function guideEntry(name: string): DocEntry {
	const sourcePath = `docs/guide/${name}`;
	const title = titleOf(sourcePath);
	if (name === 'README.md')
		return { slug: 'guide', sourcePath, section: 'guide', title, label: 'Overview' };
	const slug = `guide/${name.replace(/^\d+-/, '').replace(/\.md$/, '')}`;
	return { slug, sourcePath, section: 'guide', title, label: title };
}

function errorEntry(name: string): DocEntry {
	const sourcePath = `docs/errors/${name}`;
	const title = titleOf(sourcePath);
	if (name === 'README.md')
		return { slug: 'errors', sourcePath, section: 'errors', title, label: 'All codes' };
	const code = name.replace(/\.md$/, '');
	const label = title.replace(new RegExp(`^${code}:\\s*`), '');
	return { slug: `errors/${code}`, sourcePath, section: 'errors', title, label, code };
}

function referenceEntry(name: string): DocEntry {
	const sourcePath = `docs/reference/${name}`;
	const title = titleOf(sourcePath);
	if (name === 'README.md')
		return { slug: 'reference', sourcePath, section: 'reference', title, label: 'Overview' };
	return {
		slug: `reference/${name.replace(/\.md$/, '')}`,
		sourcePath,
		section: 'reference',
		title,
		label: title
	};
}

let cachedEntries: DocEntry[] | undefined;

/** Every docs page, in reading order. Cached for the build; re-read in dev so edits show up. */
export function docEntries(): DocEntry[] {
	if (import.meta.env.DEV) cachedEntries = undefined;
	cachedEntries ??= [
		...markdownFiles('guide').map(guideEntry),
		...markdownFiles('reference').map(referenceEntry),
		...markdownFiles('errors').map(errorEntry)
	];
	return cachedEntries;
}

export function navigation(): NavSection[] {
	const sections: SectionId[] = ['guide', 'reference', 'errors'];
	return sections
		.map((id) => ({
			id,
			title: sectionTitles[id],
			entries: docEntries()
				.filter((entry) => entry.section === id)
				.map(({ slug, label, code }) => ({ slug, label, code }))
		}))
		.filter((section) => section.entries.length > 0);
}

/** The site route of a repository file, for links between docs. */
export function routeOf(repoPath: string): string | undefined {
	const folderIndex = `${repoPath.replace(/\/$/, '')}/README.md`;
	const entry = docEntries().find(
		(candidate) => candidate.sourcePath === repoPath || candidate.sourcePath === folderIndex
	);
	return entry === undefined ? undefined : `/docs/${entry.slug}`;
}

export async function loadDocPage(slug: string): Promise<DocPage | undefined> {
	const entries = docEntries();
	const index = entries.findIndex((entry) => entry.slug === slug);
	if (index === -1) return undefined;
	const entry = entries[index];
	const source = fs.readFileSync(path.join(repoRoot, entry.sourcePath), 'utf8');
	const { html, headings, summary } = await renderMarkdown(source, entry.sourcePath, routeOf);
	const neighbour = (candidate: DocEntry | undefined) =>
		candidate && { slug: candidate.slug, title: candidate.title };
	return {
		entry,
		pageTitle: `${entry.title} · ${sectionTitles[entry.section]}`,
		description: summary || `estado documentation: ${entry.title}`,
		html,
		headings,
		previous: neighbour(entries[index - 1]),
		next: neighbour(entries[index + 1])
	};
}
