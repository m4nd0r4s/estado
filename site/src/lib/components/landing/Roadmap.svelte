<script lang="ts">
	type Status = 'done' | 'now' | 'next';
	const statusLabels: Record<Status, string> = {
		done: 'done',
		now: 'in progress',
		next: 'planned'
	};
	const milestones: { id: string; title: string; status: Status }[] = [
		{ id: 'M0', title: 'Skeleton and CI', status: 'done' },
		{ id: 'M1', title: 'Risk prototypes', status: 'done' },
		{ id: 'M2', title: 'Core by hand', status: 'done' },
		{ id: 'M3', title: 'The macro', status: 'now' },
		{ id: 'M4', title: 'Graph export and model tests', status: 'next' },
		{ id: 'M5', title: 'Tokio and Embassy runtimes', status: 'next' },
		{ id: 'M6', title: 'Snapshots and typestate', status: 'next' }
	];
</script>

<section id="roadmap">
	<div class="eyebrow">Roadmap</div>
	<h2>Built milestone by milestone, from a written spec.</h2>
	<ol>
		{#each milestones as milestone (milestone.id)}
			<li class={milestone.status} aria-current={milestone.status === 'now' ? 'step' : undefined}>
				<b>{milestone.id}</b>{milestone.title}
				<span class="visually-hidden">({statusLabels[milestone.status]})</span>
			</li>
		{/each}
	</ol>
</section>

<style>
	section {
		padding-block: 64px;
		border-top: 1px solid var(--rule);
	}
	h2 {
		font: 700 clamp(26px, 3vw, 34px)/1.12 var(--display);
		letter-spacing: -0.02em;
		margin: 10px 0 0;
	}
	ol {
		list-style: none;
		padding: 0;
		margin: 28px 0 0;
		display: grid;
		grid-template-columns: repeat(7, minmax(0, 1fr));
		gap: 8px;
	}
	li {
		border-top: 3px solid var(--rule);
		padding-top: 10px;
		font-size: 13px;
		color: var(--ink-soft);
	}
	b {
		display: block;
		font: 700 13px var(--mono);
		color: var(--ink);
	}
	.done {
		border-color: var(--ok);
	}
	.now {
		border-color: var(--rust);
	}
	.now b {
		color: var(--rust);
	}
	@media (max-width: 860px) {
		ol {
			grid-template-columns: repeat(4, minmax(0, 1fr));
		}
	}
	@media (max-width: 560px) {
		ol {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}
	}
</style>
