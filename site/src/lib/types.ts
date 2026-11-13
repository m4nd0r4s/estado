/** Types shared by server loaders and components. */

export interface NavEntry {
	slug: string;
	label: string;
	/** Leading code shown in the sidebar (`E0003`), if any. */
	code?: string;
}

export interface NavSection {
	id: 'guide' | 'reference' | 'errors';
	title: string;
	entries: NavEntry[];
}

export type DiagnosticKind = 'error' | 'help' | 'gutter' | 'caret' | 'plain';

/** One line of compiler output; `code` is the source text after a `NN |` gutter. */
export interface DiagnosticLine {
	kind: DiagnosticKind;
	text: string;
	code?: string;
}
