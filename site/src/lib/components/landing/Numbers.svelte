<script lang="ts">
	const numbers = [
		{
			value: '1.00',
			unit: '×',
			label: 'Time per event against a hand-written enum + match ping-pong.',
			source: 'BM-02 · CI x86_64'
		},
		{
			value: '0',
			unit: ' deps',
			label: 'The core is no_std, needs no allocator, and builds for Cortex-M.',
			source: 'AC-16 · thumbv7em-none-eabihf'
		},
		{
			value: '0.9',
			unit: ' s',
			label: 'cargo check of a generated machine with 200 states and 600 transitions.',
			source: 'BM-05 · local run · target 3 s'
		},
		{
			value: '1.17',
			unit: '×',
			label:
				'The macro-generated HTTP/1 parser against a hand-written one. Target 1.05×; still open.',
			source: 'BM-01 · CI x86_64 · open'
		}
	];
</script>

<section>
	<div class="eyebrow">Measured, not promised</div>
	<h2>Close to hand-written code, and we show the gap.</h2>
	<p class="note">Figures from the benchmarks in the repository, as recorded in the changelog.</p>
	<div class="numbers">
		{#each numbers as number (number.source)}
			<div class="number">
				<div class="value">{number.value}<small>{number.unit}</small></div>
				<div class="label">{number.label}</div>
				<div class="source">{number.source}</div>
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
	.numbers {
		display: grid;
		grid-template-columns: repeat(4, minmax(0, 1fr));
		border: 1px solid var(--rule);
		border-radius: 10px;
		overflow: hidden;
		background: var(--sheet);
		margin-top: 28px;
	}
	.number {
		padding: 24px;
		border-right: 1px solid var(--rule);
	}
	.number:last-child {
		border-right: 0;
	}
	.value {
		font: 800 40px/1 var(--display);
		letter-spacing: -0.03em;
		font-variant-numeric: tabular-nums;
	}
	small {
		font-size: 18px;
		color: var(--ink-soft);
		font-weight: 700;
	}
	.note {
		margin: 8px 0 0;
		color: var(--ink-soft);
		font-size: 14px;
	}
	.label {
		margin-top: 10px;
		color: var(--ink-soft);
		font-size: 14px;
	}
	.source {
		margin-top: 12px;
		font: 400 11px/1.4 var(--mono);
		color: var(--ink-soft);
	}
	@media (max-width: 860px) {
		.numbers {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}
		.number:nth-child(2) {
			border-right: 0;
		}
		.number:nth-child(-n + 2) {
			border-bottom: 1px solid var(--rule);
		}
	}
	@media (max-width: 560px) {
		.numbers {
			grid-template-columns: minmax(0, 1fr);
		}
		.number {
			border-right: 0;
			border-bottom: 1px solid var(--rule);
		}
		.number:last-child {
			border-bottom: 0;
		}
	}
</style>
