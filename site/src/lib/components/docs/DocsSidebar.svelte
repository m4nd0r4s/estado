<script lang="ts">
	import { resolve } from '$app/paths';
	import type { NavSection } from '#lib/types.ts';

	let { sections, current }: { sections: NavSection[]; current: string } = $props();
</script>

<nav class="sidebar" aria-label="Documentation">
	{#each sections as section (section.id)}
		<details open={section.id !== 'errors' || current.startsWith('errors')}>
			<summary>{section.title}</summary>
			<ul>
				{#each section.entries as entry (entry.slug)}
					<li>
						<a
							href={resolve('/docs/[...slug]', { slug: entry.slug })}
							aria-current={entry.slug === current ? 'page' : undefined}
							title={entry.label}
						>
							{#if entry.code}<code>{entry.code}</code>{/if}
							<span>{entry.label}</span>
						</a>
					</li>
				{/each}
			</ul>
		</details>
	{/each}
</nav>

<style>
	.sidebar {
		font-size: 14px;
	}
	details + details {
		margin-top: 18px;
	}
	summary {
		font: 500 12px/1 var(--mono);
		letter-spacing: 0.1em;
		text-transform: uppercase;
		color: var(--rust);
		cursor: pointer;
		padding-block: 4px;
		list-style-position: outside;
	}
	ul {
		list-style: none;
		margin: 8px 0 0;
		padding: 0;
		border-left: 1px solid var(--rule);
	}
	a {
		display: flex;
		gap: 8px;
		align-items: baseline;
		padding: 5px 10px;
		margin-left: -1px;
		border-left: 2px solid transparent;
		text-decoration: none;
		color: var(--ink-soft);
		line-height: 1.35;
	}
	a:hover {
		color: var(--ink);
	}
	a[aria-current='page'] {
		color: var(--ink);
		border-left-color: var(--rust);
		font-weight: 600;
	}
	code {
		font-size: 11.5px;
		color: var(--ink);
		flex: none;
	}
	span {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
