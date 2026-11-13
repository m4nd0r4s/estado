/**
 * A model of the README's media player, enough to replay what the generated
 * machine runs for each event (exits deepest first, entries outermost first).
 */
export type TraceKind = 'send' | 'exit' | 'entry' | 'effect' | 'note';

export interface TraceLine {
	kind: TraceKind;
	text: string;
}

export type PlayerState =
	| { name: 'Idle' }
	| { name: 'Active'; track: number; sub: 'Playing' | 'Paused'; seconds: number }
	| { name: 'Failed' };

export type PlayerEvent = 'Play' | 'Tick' | 'Pause' | 'Resume' | 'Fail';

export const initialState: PlayerState = { name: 'Idle' };

export function accepts(state: PlayerState, event: PlayerEvent): boolean {
	switch (event) {
		case 'Play':
			return state.name === 'Idle';
		case 'Tick':
		case 'Pause':
			return state.name === 'Active' && state.sub === 'Playing';
		case 'Resume':
			return state.name === 'Active' && state.sub === 'Paused';
		case 'Fail':
			return state.name === 'Active';
	}
}

export function path(state: PlayerState): string {
	return state.name === 'Active' ? `Active/${state.sub}` : state.name;
}

const line = (kind: TraceKind, text: string): TraceLine => ({ kind, text });

/** The next state and the trace of one `send`, or `undefined` when the event is not accepted. */
export function step(
	state: PlayerState,
	event: PlayerEvent
): { state: PlayerState; trace: TraceLine[] } | undefined {
	if (!accepts(state, event)) return undefined;
	if (event === 'Play') {
		return {
			state: { name: 'Active', track: 7, sub: 'Playing', seconds: 0 },
			trace: [
				line('send', 'send(Play(7))'),
				line('exit', '  exit Idle'),
				line('entry', '  entry Active'),
				line('effect', '    emit OpenTrack(7)'),
				line('entry', '  entry Playing')
			]
		};
	}
	if (state.name !== 'Active') return undefined;
	switch (event) {
		case 'Tick': {
			const seconds = state.seconds + 1;
			return {
				state: { ...state, seconds },
				trace: [line('send', 'send(Tick)'), line('note', `  stay   position = ${seconds}s`)]
			};
		}
		case 'Pause':
			return {
				state: { ...state, sub: 'Paused' },
				trace: [
					line('send', 'send(Pause)'),
					line('exit', '  exit Playing'),
					line('entry', '  entry Paused')
				]
			};
		case 'Resume':
			return {
				state: { ...state, sub: 'Playing' },
				trace: [
					line('send', 'send(Resume)'),
					line('exit', '  exit Paused'),
					line('entry', '  entry Playing')
				]
			};
		case 'Fail':
			return {
				state: { name: 'Failed' },
				trace: [
					line('send', 'send(Fail("disk"))'),
					line('note', `  ${state.sub} bubbles to Active`),
					line('exit', `  exit ${state.sub}`),
					line('exit', '  exit Active'),
					line('effect', '    emit StopOutput'),
					line('entry', '  entry Failed'),
					line('note', '  machine.is_done() == true')
				]
			};
	}
}
