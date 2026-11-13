<script lang="ts">
	import { resolve } from '$app/paths';
	import CompilerOutput from '#lib/components/landing/CompilerOutput.svelte';
	import Features from '#lib/components/landing/Features.svelte';
	import Hero from '#lib/components/landing/Hero.svelte';
	import Numbers from '#lib/components/landing/Numbers.svelte';
	import Roadmap from '#lib/components/landing/Roadmap.svelte';
	import Specimen from '#lib/components/landing/Specimen.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<svelte:head>
	<title>estado · hierarchical statecharts for Rust</title>
	<meta
		name="description"
		content="Hierarchical statecharts for Rust: states as enums, handlers as functions, every transition checked at compile time."
	/>
</svelte:head>

<Hero />

<Specimen eyebrow="Plain Rust" title="Enums for states. Functions for reactions.">
	{#snippet prose()}
		<p>
			A machine is one module. The root is an <strong>enum</strong>, compound states are
			<strong>structs</strong> that own their substates, and handlers return <strong>goto!</strong>,
			<strong>stay!</strong> or <strong>bubble!</strong>.
		</p>
		<p>
			Data moves between states by value: leaving a state can hand its data to the next one without
			a clone.
		</p>
		<p><a href={resolve('/docs/[...slug]', { slug: 'guide' })}>Start with the guide →</a></p>
	{/snippet}
	<!-- Highlighted at build time by Shiki. -->
	{@html data.playerCode}
</Specimen>

<Specimen eyebrow="Compile time" title="A wrong transition does not compile.">
	{#snippet prose()}
		<p>
			Every <strong>goto!</strong> target is resolved when the macro expands. Mistakes get a stable
			code, a precise span and a fix, and each code has a page in the
			<a href={resolve('/docs/[...slug]', { slug: 'errors' })}>error index</a>.
		</p>
		<p>This output is copied from the test suite's snapshots, not written for this page.</p>
	{/snippet}
	<CompilerOutput lines={data.compilerOutput} />
</Specimen>

<Numbers />
<Features />
<Roadmap />
