<script lang="ts">
	import '#lib/styles/global.css';
	import favicon from '#lib/assets/favicon.svg';
	import SiteHeader from '#lib/components/SiteHeader.svelte';
	import SiteFooter from '#lib/components/SiteFooter.svelte';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

<a class="skip-link" href="#content">Skip to content</a>
<div class="page">
	<header>
		<SiteHeader />
	</header>
	<main id="content" tabindex="-1">
		{@render children()}
	</main>
	<SiteFooter />
</div>
