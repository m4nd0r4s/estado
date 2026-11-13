<script lang="ts">
	import type { DiagnosticLine } from '#lib/types.ts';

	let { lines }: { lines: DiagnosticLine[] } = $props();
</script>

<!-- Scrollable, so it must take keyboard focus (WCAG 2.1.1). -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<!-- prettier-ignore -->
<pre role="region" aria-label="Compiler output" tabindex="0">{#each lines as line, index (index)}<span class="line {line.kind}">{line.text}{#if line.code !== undefined}<span class="source">{line.code}</span>{/if}</span>{/each}</pre>

<style>
	pre {
		font-size: 12.5px;
	}
	.line {
		display: block;
		min-height: 1lh;
		white-space: pre;
	}
	.error {
		color: var(--code-err);
		font-weight: 700;
	}
	.help {
		color: var(--code-help);
	}
	.gutter {
		color: var(--code-dim);
	}
	.source {
		color: var(--code-fg);
	}
	.caret {
		color: var(--code-err);
	}
</style>
