import fs from 'node:fs';
import path from 'node:path';
import { highlight } from './highlight.ts';
import { repoRoot } from './repo.ts';
import type { DiagnosticKind, DiagnosticLine } from '../types.ts';

/** An excerpt of the README's quick start; `cargo test --doc -p estado` runs the full program. */
const playerExcerpt = `#[estado::machine(event = Event, effect = Command)]
mod player {
    #[root]
    pub enum Player {
        #[initial]
        Idle,
        Active(Active),
        #[terminal]
        Failed { reason: String },
    }

    #[state]
    pub struct Active {
        pub track: u32,
        #[substates]
        pub sub: ActiveSub,
    }

    #[handlers]
    impl Player {
        #[on(Playing)]
        fn on_playing(playing: PlayingMut<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Pause => goto!(Paused { position: *playing.position }),
                _ => bubble!(), // Active handles Fail for every substate
            }
        }
    }
}`;

export type { DiagnosticLine };

/** The compile-fail snapshots the landing page quotes, verbatim. */
const snapshots = ['e0003_unknown_target.stderr', 'e0020_goto_missing_data.stderr'];

function classify(line: string): DiagnosticKind {
	if (line.startsWith('error')) return 'error';
	if (/^\s*= help:/.test(line)) return 'help';
	if (/^\s*\|\s*\^+/.test(line)) return 'caret';
	if (/^\s*(\d+\s*)?\||^\s*-->/.test(line)) return 'gutter';
	return 'plain';
}

function compilerOutput(): DiagnosticLine[] {
	const folder = path.join(repoRoot, 'crates/estado-macros/tests/ui');
	const text = snapshots
		.map((name) => fs.readFileSync(path.join(folder, name), 'utf8').trimEnd())
		.join('\n\n');
	return text.split('\n').map((line) => {
		const kind = classify(line);
		// Keep the quoted source bright; only the `NN |` gutter is dimmed.
		const source = kind === 'gutter' ? line.match(/^(\s*\d+\s*\|)(.*)$/) : null;
		return source ? { kind, text: source[1], code: source[2] } : { kind, text: line };
	});
}

/**
 * The excerpt is hand-trimmed from the README's quick start (which runs as a
 * doctest); fail the build if a line no longer appears there.
 */
function checkExcerptAgainstReadme(excerpt: string): void {
	const readme = fs.readFileSync(path.join(repoRoot, 'README.md'), 'utf8');
	const readmeLines = new Set(readme.split('\n').map((line) => line.trim()));
	const missing = excerpt
		.split('\n')
		.map((line) => line.replace(/\s*\/\/.*$/, '').trim())
		.filter((line) => line !== '' && !readmeLines.has(line));
	if (missing.length > 0) {
		throw new Error(`The landing excerpt drifted from README.md:\n${missing.join('\n')}`);
	}
}

export async function landingData() {
	checkExcerptAgainstReadme(playerExcerpt);
	return {
		playerCode: await highlight(playerExcerpt, 'rust'),
		compilerOutput: compilerOutput()
	};
}
