# Error and warning codes

One page per code (`E0001.md`, `W0001.md`, ...), with an example that triggers it, the message and the fix. Every code has a matching `trybuild` test (AC-17): errors in `crates/estado-macros/tests/ui/` (and `crates/estado/tests/ui/` for E0040), warnings in `crates/estado-macros/tests/warnings/`. The table of conditions is spec 6.7; codes added by the implementation are in `docs/spec-errata.md` (entries 8 and 9).

Every diagnostic reads `estado::<code>: <message>`, with a `help:` line when there is a fix to suggest. Warnings use rustc's `deprecated` lint (ADR 0003) and are silenced with `#[machine(allow(WNNNN))]`.

## Errors

| Code | Condition | Page |
| --- | --- | --- |
| E0001 | `#[machine]` not on an inline module with exactly one `#[root]` enum | [E0001](E0001.md) |
| E0002 | `event` not declared | [E0002](E0002.md) |
| E0010 | Level without exactly one `#[initial]` | [E0010](E0010.md) |
| E0011 | Nesting deeper than 16 levels | [E0011](E0011.md) |
| E0012 | Duplicate state name | [E0012](E0012.md) |
| E0013 | State mixes `#[substates]` and `#[region]` | [E0013](E0013.md) |
| E0014 | Parallel state with a single region | [E0014](E0014.md) |
| E0015 | Substates or region type is not a `#[state]` enum | [E0015](E0015.md) |
| E0016 | Invalid `#[machine(..)]` option, or unknown code in `allow(..)` | [E0016](E0016.md) |
| E0017 | Malformed state attribute | [E0017](E0017.md) |
| E0040 | Transition macro outside `#[handlers]` | [E0040](E0040.md) |
| E0050 | Regions in typestate mode | [E0050](E0050.md) |

## Warnings

| Code | Condition | Page | Status |
| --- | --- | --- | --- |
| W0001 | State unreachable from the initial state | [W0001](W0001.md) | analysis done, enabled with the edges |
| W0002 | State with no outgoing edges that is not `#[terminal]` | [W0002](W0002.md) | analysis done, enabled with the edges |
| W0004 | Variant larger than `size_warn` bytes | [W0004](W0004.md) | emitted |

## Not yet implemented

Pages are added with each code: E0003-E0006, E0020-E0022, E0030, E0031, E0041, E0060, E0061, W0003, W0005, W0010.
