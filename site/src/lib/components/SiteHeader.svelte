<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { repositoryUrl } from '#lib/site.ts';
	import ThemeToggle from './ThemeToggle.svelte';

	const links = [
		{ href: resolve('/docs/[...slug]', { slug: 'guide' }), label: 'Guide' },
		{ href: resolve('/docs/[...slug]', { slug: 'reference' }), label: 'Reference' },
		{ href: resolve('/docs/[...slug]', { slug: 'errors' }), label: 'Error index' }
	];

	function isCurrent(href: string): boolean {
		const path = page.url.pathname.replace(/\/$/, '');
		return path === href || path.startsWith(`${href}/`);
	}
</script>

<nav class="nav" aria-label="Main">
	<a class="wordmark" href={resolve('/')}>
		<svg viewBox="0 0 22 22" aria-hidden="true">
			<rect
				x="1.5"
				y="5"
				width="19"
				height="12"
				rx="6"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
			/>
			<circle cx="7.5" cy="11" r="2.6" fill="var(--rust)" />
		</svg>
		estado
	</a>
	<span class="badge">v0 · pre-release</span>
	<div class="links">
		{#each links as link (link.href)}
			<a href={link.href} aria-current={isCurrent(link.href) ? 'page' : undefined}>{link.label}</a>
		{/each}
		<a href={repositoryUrl}>GitHub</a>
	</div>
	<ThemeToggle />
</nav>

<style>
	.nav {
		display: flex;
		align-items: center;
		gap: 20px;
		padding-block: 20px;
		border-bottom: 1px solid var(--rule);
	}
	.wordmark {
		font: 800 22px/1 var(--display);
		letter-spacing: -0.02em;
		text-decoration: none;
		display: flex;
		align-items: center;
		gap: 8px;
	}
	.wordmark svg {
		width: 22px;
		height: 22px;
	}
	.badge {
		font: 500 11px/1 var(--mono);
		text-transform: uppercase;
		letter-spacing: 0.08em;
		padding: 5px 8px;
		border: 1px solid var(--rule);
		border-radius: 4px;
		color: var(--ink-soft);
		white-space: nowrap;
	}
	.links {
		display: flex;
		gap: 20px;
		margin-left: auto;
		font-size: 14px;
		flex-wrap: wrap;
	}
	.links a {
		text-decoration: none;
		color: var(--ink-soft);
	}
	.links a:hover,
	.links a[aria-current='page'] {
		color: var(--ink);
	}
	.links a[aria-current='page'] {
		text-decoration: underline;
		text-decoration-color: var(--rust);
		text-underline-offset: 6px;
		text-decoration-thickness: 2px;
	}
	@media (max-width: 760px) {
		.nav {
			flex-wrap: wrap;
			gap: 12px 16px;
		}
		.links {
			order: 3;
			width: 100%;
			margin-left: 0;
			gap: 16px;
		}
		.badge {
			margin-right: auto;
		}
	}
</style>
