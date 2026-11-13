# Error and warning codes

One page per code (`E0001.md`, `W0001.md`, ...), with an example that triggers it, the message and the fix. Every code has a matching `trybuild` test: errors in `crates/estado-macros/tests/ui/` (and `crates/estado/tests/ui/` for E0040), warnings in `crates/estado-macros/tests/warnings/`.

Every diagnostic reads `estado::<code>: <message>`, with a `help:` line when there is a fix to suggest. Warnings use rustc's `deprecated` lint and are silenced with `#[machine(allow(WNNNN))]`.

## Errors

| Code | Condition | Page |
| --- | --- | --- |
| E0001 | `#[machine]` not on an inline module with exactly one `#[root]` enum | [E0001](E0001.md) |
| E0002 | `event` not declared | [E0002](E0002.md) |
| E0003 | `goto!` target is not a state | [E0003](E0003.md) |
| E0004 | `#[on(S)]` (or another role) with an unknown state | [E0004](E0004.md) |
| E0005 | Duplicate handler for a role, or two roles on one function | [E0005](E0005.md) |
| E0006 | Handler parameter not recognised, out of order or repeated | [E0006](E0006.md) |
| E0010 | Level without exactly one `#[initial]` | [E0010](E0010.md) |
| E0011 | Nesting deeper than 16 levels | [E0011](E0011.md) |
| E0012 | Duplicate state name, or a name equal to its own id constant | [E0012](E0012.md) |
| E0013 | State mixes `#[substates]` and `#[region]` | [E0013](E0013.md) |
| E0014 | Parallel state with a single region | [E0014](E0014.md) |
| E0015 | Substates or region type is not a `#[state]` enum | [E0015](E0015.md) |
| E0016 | Invalid `#[machine(..)]` option, or unknown code in `allow(..)` | [E0016](E0016.md) |
| E0017 | Malformed state attribute, or a marker on the wrong item | [E0017](E0017.md) |
| E0020 | `goto!` or `#[initial]` without the data a state needs | [E0020](E0020.md) |
| E0021 | Transition between sibling regions | [E0021](E0021.md) |
| E0022 | A `#[terminal]` state transitions out | [E0022](E0022.md) |
| E0040 | Transition macro outside `#[handlers]` | [E0040](E0040.md) |
| E0042 | Transition in an `#[entry]` or `#[exit]` | [E0042](E0042.md) |
| E0043 | Malformed `goto!` | [E0043](E0043.md) |
| E0044 | `#[on_done]` on a state that never completes | [E0044](E0044.md) |
| E0045 | `reenter!`, `defer!`, `bubble!` or `unhandled!` in `#[always]` or `#[on_done]` | [E0045](E0045.md) |
| E0046 | Constructor uses what the handler borrows | [E0046](E0046.md) |
| E0050 | Regions in typestate mode | [E0050](E0050.md) |
| E0060 | `async` handler | [E0060](E0060.md) |
| E0061 | `Result` handler without `#[on_error]` | [E0061](E0061.md) |
| E0090 | Feature not supported yet | [E0090](E0090.md) |

## Warnings

| Code | Condition | Page | Status |
| --- | --- | --- | --- |
| W0001 | State unreachable from the initial state | [W0001](W0001.md) | emitted |
| W0002 | State with no outgoing edges that is not `#[terminal]` | [W0002](W0002.md) | emitted |
| W0003 | `defer!()` in a state with no outgoing edges | [W0003](W0003.md) | emitted |
| W0004 | Variant larger than `size_warn` bytes | [W0004](W0004.md) | emitted |
| W0005 | `#[after]` event that no handler handles (best effort) | [W0005](W0005.md) | emitted |
| W0010 | Typestate `match` arm not associated with an event variant | W0010 | M6 (needs typestate) |

## Not yet implemented

Pages are added with each code: E0030, E0031, E0041, W0010 (the last one belongs to milestone M6, typestate).
