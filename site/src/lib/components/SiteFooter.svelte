<script lang="ts">
	import { repositoryUrl } from '#lib/site.ts';
</script>

<footer>
	<span>estado · MIT or Apache-2.0</span>
	<a href={repositoryUrl}>GitHub</a>
	<a href="{repositoryUrl}/blob/main/CHANGELOG.md">Changelog</a>
	<span class="spacer"
		>The docs and the compiler output on this site are built from the repository; their code samples
		run as tests.</span
	>
</footer>

<style>
	footer {
		border-top: 1px solid var(--rule);
		padding-block: 28px 40px;
		display: flex;
		gap: 8px 16px;
		flex-wrap: wrap;
		color: var(--ink-soft);
		font-size: 14px;
	}
	.spacer {
		margin-left: auto;
		max-width: 52ch;
	}
	@media (max-width: 760px) {
		.spacer {
			margin-left: 0;
		}
	}
</style>
