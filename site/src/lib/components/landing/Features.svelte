<script lang="ts">
	const features = [
		{
			title: 'Hierarchy',
			tag: '#[substates]',
			text: "Substates read and write their parent's fields through generated views. Exits run deepest first, entries outermost first."
		},
		{
			title: 'Parallel regions',
			tag: '#[region]',
			text: 'Regions run in declaration order and exit in reverse. A region can ask what its siblings are doing with cx.in_state.'
		},
		{
			title: 'Timers and tasks',
			tag: '#[after]',
			text: 'Timers and invokes belong to a state instance. A timeout that fires after the state left is dropped as stale.'
		},
		{
			title: 'History',
			tag: '#[history]',
			text: 'Return to the substate you left, shallow or with its full data, without cloning.'
		},
		{
			title: 'Defer and raise',
			tag: 'defer!()',
			text: 'Postpone events until the configuration changes, or queue internal events that run before the next external one.'
		},
		{
			title: 'Panic safety',
			tag: 'Poisoned',
			text: 'A panic in the middle of a transition poisons the machine. It never leaves a half-built state behind.'
		}
	];
</script>

<section>
	<div class="eyebrow">What it covers</div>
	<h2>The parts of a statechart you end up needing.</h2>
	<div class="features">
		{#each features as feature (feature.title)}
			<div class="feature">
				<h3>{feature.title} <code>{feature.tag}</code></h3>
				<p>{feature.text}</p>
			</div>
		{/each}
	</div>
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
	.features {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 28px 32px;
		margin-top: 32px;
	}
	h3 {
		font: 700 17px/1.3 var(--display);
		margin: 0 0 6px;
		display: flex;
		gap: 10px;
		align-items: baseline;
		flex-wrap: wrap;
	}
	code {
		font-size: 12px;
		color: var(--rust);
	}
	p {
		margin: 0;
		color: var(--ink-soft);
		font-size: 15px;
	}
	@media (max-width: 860px) {
		.features {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}
	}
	@media (max-width: 560px) {
		.features {
			grid-template-columns: minmax(0, 1fr);
		}
	}
</style>
