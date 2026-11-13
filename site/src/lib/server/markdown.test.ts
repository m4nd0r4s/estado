import assert from 'node:assert/strict';
import { test } from 'node:test';
import { withoutHiddenLines } from './highlight.ts';
import { renderMarkdown, resolveLink, slugify } from './markdown.ts';

const routes = new Map([
	['docs/guide/README.md', '/docs/guide'],
	['docs/errors/E0003.md', '/docs/errors/E0003']
]);
const routeOf = (repoPath: string) => routes.get(repoPath);

test('slugs match the anchors GitHub generates', () => {
	assert.equal(slugify('goto!'), 'goto');
	assert.equal(slugify('#[on_done]'), 'on_done');
	assert.equal(slugify('A & B'), 'a--b');
	assert.equal(slugify('Constructor form'), 'constructor-form');
});

test('links to docs become routes, other files go to GitHub, folders use their README', () => {
	const from = 'docs/reference/macros.md';
	assert.equal(resolveLink('../errors/E0003.md#when', from, routeOf), '/docs/errors/E0003#when');
	assert.equal(
		resolveLink('../guide/', from, (path) => routeOf(`${path}/README.md`)),
		'/docs/guide'
	);
	assert.match(
		resolveLink('../../README.md', from, routeOf),
		/github\.com\/.+\/blob\/main\/README\.md$/
	);
	assert.equal(resolveLink('#goto', from, routeOf), '#goto');
	assert.equal(resolveLink('https://example.com', from, routeOf), 'https://example.com');
});

test('broken and unsafe links fail the build', () => {
	assert.throws(() => resolveLink('./missing.md', 'docs/guide/README.md', routeOf), /broken link/);
	assert.throws(
		() => resolveLink('javascript:alert(1)', 'docs/guide/README.md', routeOf),
		/unsafe/
	);
});

test('headings get plain-text ids and table-of-contents entries', async () => {
	const { headings } = await renderMarkdown(
		'# Title\n\n## `goto!` & friends\n\n## Again\n\n## Again\n',
		'docs/guide/README.md',
		routeOf
	);
	assert.deepEqual(
		headings.map((heading) => [heading.id, heading.text]),
		[
			['goto--friends', 'goto! & friends'],
			['again', 'Again'],
			['again-1', 'Again']
		]
	);
});

test('raw HTML other than inline tags fails the build', async () => {
	await assert.rejects(
		renderMarkdown('<script>alert(1)</script>\n', 'docs/guide/README.md', routeOf),
		/raw HTML/
	);
});

test('rustdoc hidden lines are dropped and ## unescaped', () => {
	assert.equal(
		withoutHiddenLines('# use x;\n#[derive(Debug)]\nlet a = 1;\n## not hidden'),
		'#[derive(Debug)]\nlet a = 1;\n# not hidden'
	);
});
