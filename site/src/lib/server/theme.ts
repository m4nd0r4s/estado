import type { ThemeRegistration } from 'shiki';

/** Ink-on-dark code theme matching the site palette (code blocks are dark in both themes). */
export const inkTheme: ThemeRegistration = {
	name: 'estado-ink',
	type: 'dark',
	colors: { 'editor.background': '#121a25', 'editor.foreground': '#d9e1ec' },
	tokenColors: [
		{ settings: { foreground: '#d9e1ec' } },
		{ scope: ['comment', 'punctuation.definition.comment'], settings: { foreground: '#7f8ea3' } },
		{
			scope: ['keyword', 'storage', 'storage.type', 'storage.modifier', 'keyword.operator.borrow'],
			settings: { foreground: '#f0a97a' }
		},
		{ scope: ['string', 'constant.character'], settings: { foreground: '#9fd3a8' } },
		{ scope: ['constant.numeric', 'constant.language'], settings: { foreground: '#e6c07b' } },
		{
			scope: ['entity.name.function.macro', 'support.function.macro', 'meta.attribute'],
			settings: { foreground: '#9cc4f5' }
		},
		{
			scope: ['entity.name.type', 'support.type', 'entity.name.type.enum'],
			settings: { foreground: '#c7b8f5' }
		},
		{ scope: ['entity.name.function'], settings: { foreground: '#e4eaf2' } },
		{ scope: ['markup.inserted'], settings: { foreground: '#9fd3a8' } },
		{ scope: ['markup.deleted'], settings: { foreground: '#f2a7a7' } }
	]
};
