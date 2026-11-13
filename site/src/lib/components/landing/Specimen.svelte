<script lang="ts">
	import type { Snippet } from 'svelte';

	let {
		id,
		eyebrow,
		title,
		prose,
		children
	}: { id?: string; eyebrow: string; title: string; prose: Snippet; children: Snippet } = $props();
</script>

<!-- Prose on the left, a specimen (code, compiler output) on the right. -->
<section {id}>
	<div class="split">
		<div class="text">
			<div class="eyebrow">{eyebrow}</div>
			<h2>{title}</h2>
			{@render prose()}
		</div>
		<div class="specimen">{@render children()}</div>
	</div>
</section>

<style>
	section {
		padding-block: 64px;
		border-top: 1px solid var(--rule);
	}
	.split {
		display: grid;
		grid-template-columns: minmax(0, 4fr) minmax(0, 8fr);
		gap: 48px;
		align-items: start;
	}
	h2 {
		font: 700 clamp(26px, 3vw, 34px)/1.12 var(--display);
		letter-spacing: -0.02em;
		margin: 10px 0 14px;
		text-wrap: balance;
	}
	.text :global(p) {
		color: var(--ink-soft);
		margin: 0 0 14px;
		max-width: 46ch;
	}
	.text :global(strong) {
		color: var(--ink);
		font-weight: 600;
	}
	.specimen {
		min-width: 0;
	}
	@media (max-width: 860px) {
		.split {
			grid-template-columns: minmax(0, 1fr);
			gap: 24px;
		}
	}
</style>
