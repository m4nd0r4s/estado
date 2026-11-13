<script lang="ts">
	import { page } from '$app/state';
	import DocsSidebar from '#lib/components/docs/DocsSidebar.svelte';
	import type { LayoutProps } from './$types';

	let { data, children }: LayoutProps = $props();
	const current = $derived(page.params.slug ?? '');
	let mobileMenu = $state<HTMLDetailsElement>();

	// The layout survives client navigation; close the mobile menu on each page change.
	$effect(() => {
		void page.url.pathname;
		if (mobileMenu) mobileMenu.open = false;
	});
</script>

<div class="docs">
	<div class="content">
		{@render children()}
	</div>
	<aside aria-label="Documentation menu">
		<details class="mobile-menu" bind:this={mobileMenu}>
			<summary>Menu</summary>
			<DocsSidebar sections={data.navigation} {current} />
		</details>
		<div class="desktop-menu">
			<DocsSidebar sections={data.navigation} {current} />
		</div>
	</aside>
</div>

<style>
	/* The article comes first in the DOM so keyboard users reach it before ~40 menu links. */
	.docs {
		display: grid;
		grid-template-columns: 230px minmax(0, 1fr);
		grid-template-areas: 'menu content';
		gap: 48px;
		padding-block: 32px 64px;
	}
	aside {
		grid-area: menu;
		min-width: 0;
	}
	.content {
		grid-area: content;
		min-width: 0;
	}
	.desktop-menu {
		position: sticky;
		top: 24px;
		max-height: calc(100vh - 48px);
		overflow-y: auto;
		padding: 4px 8px 4px 4px;
	}
	.mobile-menu {
		display: none;
	}
	@media (max-width: 860px) {
		.docs {
			grid-template-columns: minmax(0, 1fr);
			grid-template-areas: 'menu' 'content';
			gap: 20px;
			padding-top: 16px;
		}
		.desktop-menu {
			display: none;
		}
		.mobile-menu {
			display: block;
			border: 1px solid var(--rule);
			border-radius: 8px;
			background: var(--sheet);
			padding: 10px 14px;
		}
		.mobile-menu > summary {
			font: 600 14px/1.4 var(--body);
			cursor: pointer;
		}
		.mobile-menu[open] > summary {
			margin-bottom: 12px;
		}
	}
</style>
