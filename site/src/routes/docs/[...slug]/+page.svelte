<script lang="ts">
	import { resolve } from '$app/paths';
	import '#lib/components/docs/prose.css';
	import { repositoryUrl } from '#lib/site.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const tableOfContents = $derived(data.headings.filter((heading) => heading.depth === 2));
</script>

<svelte:head>
	<title>{data.pageTitle} · estado</title>
	<meta name="description" content={data.description} />
</svelte:head>

<div class="layout">
	<article>
		<!-- Rendered at build time from the repository's own Markdown. -->
		<div class="prose">{@html data.html}</div>

		<nav class="pager" aria-label="Previous and next page">
			{#if data.previous}
				<a class="previous" href={resolve('/docs/[...slug]', { slug: data.previous.slug })}>
					<small>Previous</small>{data.previous.title}
				</a>
			{/if}
			{#if data.next}
				<a class="next" href={resolve('/docs/[...slug]', { slug: data.next.slug })}>
					<small>Next</small>{data.next.title}
				</a>
			{/if}
		</nav>
		<p class="edit">
			<a href="{repositoryUrl}/edit/main/{data.entry.sourcePath}">Edit this page on GitHub</a>
		</p>
	</article>

	{#if tableOfContents.length > 1}
		<nav class="toc" aria-label="On this page">
			<div class="toc-title">On this page</div>
			<ul>
				{#each tableOfContents as heading (heading.id)}
					<li><a href="#{heading.id}">{heading.text}</a></li>
				{/each}
			</ul>
		</nav>
	{/if}
</div>

<style>
	.layout {
		display: grid;
		grid-template-columns: minmax(0, 1fr) 200px;
		gap: 40px;
		align-items: start;
		min-width: 0;
	}
	article {
		min-width: 0;
		max-width: 76ch;
	}
	.toc {
		position: sticky;
		top: 24px;
		font-size: 13px;
	}
	.toc-title {
		font: 500 12px/1 var(--mono);
		letter-spacing: 0.1em;
		text-transform: uppercase;
		color: var(--ink-soft);
		margin-bottom: 10px;
	}
	.toc ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}
	.toc li + li {
		margin-top: 6px;
	}
	.toc a {
		color: var(--ink-soft);
		text-decoration: none;
	}
	.toc a:hover {
		color: var(--ink);
	}
	.pager {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 12px;
		margin-top: 48px;
	}
	.pager a {
		display: block;
		padding: 14px 16px;
		border: 1px solid var(--rule);
		border-radius: 8px;
		background: var(--sheet);
		text-decoration: none;
		font-weight: 600;
	}
	.pager a:hover {
		border-color: var(--ink-soft);
	}
	.pager small {
		display: block;
		font: 500 11px/1.6 var(--mono);
		text-transform: uppercase;
		letter-spacing: 0.08em;
		color: var(--ink-soft);
	}
	.next {
		grid-column: 2;
		text-align: right;
	}
	.edit {
		margin-top: 24px;
		font-size: 13px;
		color: var(--ink-soft);
	}
	@media (max-width: 1100px) {
		.layout {
			grid-template-columns: minmax(0, 1fr);
		}
		.toc {
			display: none;
		}
	}
	@media (max-width: 560px) {
		.pager {
			grid-template-columns: minmax(0, 1fr);
		}
		.next {
			grid-column: 1;
		}
	}
</style>
