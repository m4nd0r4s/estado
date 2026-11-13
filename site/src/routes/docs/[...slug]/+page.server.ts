import { error } from '@sveltejs/kit';
import { docEntries, loadDocPage } from '#lib/server/docs.ts';
import type { EntryGenerator, PageServerLoad } from './$types';

export const entries: EntryGenerator = () => docEntries().map((entry) => ({ slug: entry.slug }));

export const load: PageServerLoad = async ({ params }) => {
	const page = await loadDocPage(params.slug);
	if (page === undefined) error(404, `No docs page at /docs/${params.slug}`);
	return page;
};
