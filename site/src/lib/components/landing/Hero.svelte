<script lang="ts">
	import { resolve } from '$app/paths';
	import { installLine, repositoryUrl } from '#lib/site.ts';
	import PlayerLab from './PlayerLab.svelte';
</script>

<section class="hero" aria-labelledby="hero-title">
	<div>
		<div class="eyebrow">Hierarchical statecharts for Rust</div>
		<h1 id="hero-title">State machines the compiler can check.</h1>
		<p class="lede">
			Write states as Rust enums and handlers as plain functions. estado checks every transition at
			compile time and generates the dispatch, the exit and entry order, and the graph.
		</p>
		<div class="actions">
			<a class="button primary" href={resolve('/docs/[...slug]', { slug: 'guide' })}
				>Read the guide</a
			>
			<a class="button ghost" href={repositoryUrl}>View on GitHub</a>
		</div>
		<p class="install">Cargo.toml · <code>{installLine}</code></p>
	</div>
	<PlayerLab />
</section>

<style>
	.hero {
		display: grid;
		grid-template-columns: minmax(0, 5fr) minmax(0, 7fr);
		gap: 48px;
		padding-block: 56px 40px;
		align-items: start;
	}
	h1 {
		font: 800 clamp(36px, 5vw, 56px)/1.04 var(--display);
		letter-spacing: -0.03em;
		margin: 14px 0 18px;
		text-wrap: balance;
	}
	.lede {
		color: var(--ink-soft);
		font-size: 18px;
		max-width: 34ch;
		margin: 0 0 24px;
	}
	.actions {
		display: flex;
		gap: 12px;
		flex-wrap: wrap;
	}
	.button {
		font: 600 15px/1 var(--body);
		padding: 13px 18px;
		border-radius: 6px;
		text-decoration: none;
		border: 1px solid var(--ink);
	}
	.primary {
		background: var(--ink);
		color: var(--paper);
	}
	.ghost {
		color: var(--ink);
	}
	.install {
		margin-top: 22px;
		font: 400 13px/1.5 var(--mono);
		color: var(--ink-soft);
		overflow-wrap: anywhere;
	}
	.install code {
		color: var(--ink);
		background: var(--sheet);
		border: 1px solid var(--rule);
		padding: 3px 6px;
		border-radius: 4px;
	}
	@media (max-width: 1000px) {
		.hero {
			grid-template-columns: minmax(0, 1fr);
			gap: 28px;
			padding-top: 36px;
		}
	}
</style>
