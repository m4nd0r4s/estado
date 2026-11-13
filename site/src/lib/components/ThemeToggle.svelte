<script lang="ts">
	type Theme = 'system' | 'light' | 'dark';
	const order: Theme[] = ['system', 'light', 'dark'];
	const storageKey = 'estado-theme';

	let theme = $state<Theme>('system');

	$effect(() => {
		try {
			const saved = localStorage.getItem(storageKey);
			if (saved === 'light' || saved === 'dark') theme = saved;
		} catch {
			// Storage can be blocked; the system theme still applies.
		}
	});

	function cycle() {
		theme = order[(order.indexOf(theme) + 1) % order.length];
		const root = document.documentElement;
		if (theme === 'system') delete root.dataset.theme;
		else root.dataset.theme = theme;
		try {
			if (theme === 'system') localStorage.removeItem(storageKey);
			else localStorage.setItem(storageKey, theme);
		} catch {
			// Not persisted; the choice lasts for this page.
		}
	}
</script>

<button type="button" class="toggle" onclick={cycle} aria-label="Theme: {theme}. Change theme">
	<svg viewBox="0 0 16 16" aria-hidden="true">
		{#if theme === 'light'}
			<circle cx="8" cy="8" r="3.2" />
			<path d="M8 1v2M8 13v2M1 8h2M13 8h2M3 3l1.4 1.4M11.6 11.6 13 13M3 13l1.4-1.4M11.6 4.4 13 3" />
		{:else if theme === 'dark'}
			<path d="M12.5 10.2A5.5 5.5 0 0 1 5.8 3.5a5.5 5.5 0 1 0 6.7 6.7Z" />
		{:else}
			<circle cx="8" cy="8" r="5.5" />
			<path d="M8 2.5v11" />
			<path class="fill" d="M8 2.5a5.5 5.5 0 0 1 0 11Z" />
		{/if}
	</svg>
	<span class="label">{theme}</span>
</button>

<style>
	.toggle {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		font: 500 12px/1 var(--mono);
		color: var(--ink-soft);
		background: transparent;
		border: 1px solid var(--rule);
		border-radius: 6px;
		padding: 6px 8px;
		cursor: pointer;
	}
	.toggle:hover {
		color: var(--ink);
		border-color: var(--ink-soft);
	}
	svg {
		width: 14px;
		height: 14px;
		fill: none;
		stroke: currentColor;
		stroke-width: 1.4;
		stroke-linecap: round;
	}
	.fill {
		fill: currentColor;
		stroke: none;
	}
	@media (max-width: 560px) {
		.label {
			display: none;
		}
	}
</style>
