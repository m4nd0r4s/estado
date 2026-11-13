<script lang="ts">
	import { tick } from 'svelte';
	import {
		accepts,
		initialState,
		path,
		step,
		type PlayerEvent,
		type PlayerState,
		type TraceLine
	} from './player.ts';

	const intro: TraceLine[] = [
		{ kind: 'note', text: '// send an event; this is what the machine runs' },
		{ kind: 'entry', text: 'entry Idle' }
	];
	const events: { event: PlayerEvent; label: string }[] = [
		{ event: 'Play', label: 'send Play(7)' },
		{ event: 'Tick', label: 'send Tick' },
		{ event: 'Pause', label: 'send Pause' },
		{ event: 'Resume', label: 'send Resume' },
		{ event: 'Fail', label: 'send Fail' }
	];

	/** Enough for a long session; older lines scroll away. */
	const maxTraceLines = 200;

	let machine = $state.raw<PlayerState>(initialState);
	let trace = $state.raw<TraceLine[]>([...intro]);
	let traceElement: HTMLDivElement;

	const activeStates = $derived(
		new Set(machine.name === 'Active' ? ['Active', machine.sub] : [machine.name])
	);
	const position = (sub: 'Playing' | 'Paused') =>
		machine.name === 'Active' && machine.sub === sub
			? `position: ${machine.seconds}s`
			: 'position: —';

	async function send(event: PlayerEvent) {
		const result = step(machine, event);
		if (result === undefined) return;
		machine = result.state;
		trace = [...trace, ...result.trace].slice(-maxTraceLines);
		await tick();
		traceElement.scrollTop = traceElement.scrollHeight;
	}

	function reset() {
		machine = initialState;
		trace = [...intro];
	}
</script>

<div class="lab" role="region" aria-label="Interactive statechart of a media player">
	<div class="head">
		<span>README · media player · live</span>
		<span class="path">machine.path() = <b>{path(machine)}</b></span>
	</div>
	<!-- Scrollable, so it must take keyboard focus (WCAG 2.1.1). -->
	<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
	<div class="chart" role="region" aria-label="Statechart diagram" tabindex="0">
		<svg
			viewBox="0 0 620 300"
			role="img"
			aria-label="Player: Idle, Active with Playing and Paused, Failed"
		>
			<defs>
				<marker
					id="arrow"
					viewBox="0 0 10 10"
					refX="9"
					refY="5"
					markerWidth="7"
					markerHeight="7"
					orient="auto-start-reverse"
				>
					<path class="arrow-head" d="M0 0 L10 5 L0 10 z" />
				</marker>
			</defs>
			<circle class="initial" cx="24" cy="70" r="6" />
			<path class="edge" d="M30 70 H52" />

			<g class="node" class:active={activeStates.has('Idle')}>
				<rect x="54" y="46" width="110" height="48" rx="12" />
				<text class="name" x="109" y="75" text-anchor="middle">Idle</text>
			</g>

			<g class="node" class:active={activeStates.has('Active')}>
				<rect x="234" y="20" width="250" height="210" rx="16" />
				<text class="name" x="252" y="44">Active</text>
				<text class="sub" x="252" y="62">track: u32</text>
			</g>
			<circle class="initial" cx="262" cy="104" r="5" />
			<path class="edge" d="M267 104 H286" />
			<g class="node" class:active={activeStates.has('Playing')}>
				<rect x="288" y="80" width="176" height="48" rx="12" />
				<text class="name" x="376" y="104" text-anchor="middle">Playing</text>
				<text class="sub" x="376" y="120" text-anchor="middle">{position('Playing')}</text>
			</g>
			<g class="node" class:active={activeStates.has('Paused')}>
				<rect x="288" y="164" width="176" height="48" rx="12" />
				<text class="name" x="376" y="188" text-anchor="middle">Paused</text>
				<text class="sub" x="376" y="204" text-anchor="middle">{position('Paused')}</text>
			</g>
			<path class="edge" d="M350 128 V162" />
			<text class="edge-label" x="306" y="150">Pause</text>
			<path class="edge" d="M402 162 V130" />
			<text class="edge-label" x="410" y="150">Resume</text>

			<g class="node" class:active={activeStates.has('Failed')}>
				<rect x="530" y="232" width="84" height="48" rx="12" />
				<rect class="inner-border" x="534" y="236" width="76" height="40" rx="9" />
				<text class="name" x="572" y="261" text-anchor="middle">Failed</text>
			</g>

			<path class="edge" d="M164 64 H232" />
			<text class="edge-label" x="172" y="56">Play(n)</text>
			<path class="edge" d="M484 200 C 520 200, 560 210, 568 230" />
			<text class="edge-label" x="500" y="192">Fail</text>
			<text class="edge-label" x="54" y="284">double border = #[terminal]</text>
		</svg>
	</div>
	<div class="events" role="group" aria-label="Send an event">
		{#each events as { event, label } (event)}
			<button type="button" disabled={!accepts(machine, event)} onclick={() => send(event)}
				>{label}</button
			>
		{/each}
		<button type="button" class="reset" onclick={reset}>reset</button>
	</div>
	<!-- Scrollable, so it must take keyboard focus (WCAG 2.1.1). -->
	<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
	<div class="trace" bind:this={traceElement} role="log" aria-label="Trace" tabindex="0">
		{#each trace as line, index (index)}
			<span class="line {line.kind}">{line.text}</span>
		{/each}
	</div>
</div>

<style>
	.lab {
		background: var(--sheet);
		border: 1px solid var(--rule);
		border-radius: 10px;
		overflow: hidden;
		min-width: 0;
	}
	.head {
		display: flex;
		align-items: center;
		gap: 10px;
		flex-wrap: wrap;
		padding: 12px 16px;
		border-bottom: 1px solid var(--rule);
		font: 500 12px/1.3 var(--mono);
		color: var(--ink-soft);
	}
	.path {
		margin-left: auto;
		color: var(--ink);
	}
	.path b {
		color: var(--rust);
		font-weight: 700;
	}
	.chart {
		background-color: var(--sheet);
		background-image:
			linear-gradient(var(--grid) 1px, transparent 1px),
			linear-gradient(90deg, var(--grid) 1px, transparent 1px);
		background-size: 20px 20px;
		padding: 12px;
		overflow-x: auto;
	}
	svg {
		display: block;
		width: 100%;
		min-width: 440px;
		height: auto;
	}
	.node rect {
		fill: var(--sheet);
		stroke: var(--ink-soft);
		stroke-width: 1.5;
		transition:
			fill 0.18s ease,
			stroke 0.18s ease;
	}
	.node rect.inner-border {
		fill: none;
	}
	.node text {
		fill: var(--ink);
		font: 600 14px var(--display);
	}
	.node .sub {
		fill: var(--ink-soft);
		font: 400 11px var(--mono);
	}
	.node.active rect {
		fill: var(--rust-soft);
		stroke: var(--rust);
		stroke-width: 2;
	}
	.node.active rect.inner-border {
		fill: none;
	}
	.node.active .name {
		fill: var(--rust);
	}
	.edge {
		stroke: var(--ink-soft);
		stroke-width: 1.4;
		fill: none;
		marker-end: url(#arrow);
	}
	.edge-label {
		fill: var(--ink-soft);
		font: 400 11px var(--mono);
	}
	.initial {
		fill: var(--ink);
	}
	.arrow-head {
		fill: var(--ink-soft);
	}
	.events {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
		padding: 14px 16px;
		border-top: 1px solid var(--rule);
	}
	.events button {
		font: 500 13px/1 var(--mono);
		padding: 9px 12px;
		border-radius: 6px;
		border: 1px solid var(--rule);
		background: var(--paper);
		color: var(--ink);
		cursor: pointer;
	}
	.events button:hover:not(:disabled) {
		border-color: var(--ink-soft);
	}
	.events button:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}
	.events .reset {
		margin-left: auto;
		background: transparent;
	}
	.trace {
		background: var(--code-bg);
		color: var(--code-fg);
		font: 400 12.5px/1.7 var(--mono);
		padding: 14px 16px;
		height: 156px;
		overflow-y: auto;
	}
	.line {
		display: block;
		white-space: pre;
	}
	.send {
		color: var(--code-key);
	}
	.exit {
		color: var(--code-exit);
	}
	.entry {
		color: var(--code-str);
	}
	.effect {
		color: var(--code-fx);
	}
	.note {
		color: var(--code-dim);
	}
</style>
