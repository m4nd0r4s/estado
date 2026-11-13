# The estado guide

Task-oriented pages for people who build state machines with `estado`. Read them in order the first time, or jump to the page you need. Every code sample runs as a test.

For exhaustive lists of attributes, macros, methods and options, see the reference pages in [`../reference/`](../reference/attributes.md). The compile-time error and warning codes are in the [error index](../errors/README.md).

## Pages

| Page | What it covers |
| --- | --- |
| [Getting started](./01-getting-started.md) | Install, a first machine, `Machine::new`, `send`, `path()`, `state()`, `is_done()`, steps and effects. |
| [States](./02-states.md) | `#[root]`, `#[state]`, `#[initial]` and `#[initial(with = ..)]`, compound states with `#[substates]`, `#[terminal]` and the machine output. |
| [State data and views](./03-state-data.md) | Data in states, the generated `XFields`, `XMut` and `XRef` views, `reborrow`, state ids and the static graph. |
| [Handlers](./04-handlers.md) | `#[handlers]`, `#[on]`, handler parameters, `stay!`, `bubble!`, event dispatch and bubbling order. |
| [Transitions](./05-transitions.md) | `goto!` in its simple, path and constructor forms. |
| [Transition kinds](./06-transition-kinds.md) | Local and external transitions, `history`, `reenter!`, and the LCA table. |
| [Unhandled events](./07-unhandled-events.md) | `unhandled!`, the `ignore`, `error` and `panic` policies, and `#[unhandled]`. |
| [Entry and exit](./08-entry-exit.md) | `#[entry]`, `#[exit]`, the order of exits and entries, entries on start. |
| [Always](./09-always.md) | `#[always]` automatic transitions. |
| [On done](./10-on-done.md) | `#[on_done]` and completion of compound states. |
| [Context and effects](./11-context-and-effects.md) | `ctx = ..`, `Cx` (`ctx`, `ctx_mut`, `emit`, `raise`, `in_state`), effects. |
| [Sinks and drivers](./12-sinks-and-drivers.md) | `send_into` and the other `_into` forms, custom sinks, `Driver` and `drive`. |
| [Regions](./13-regions.md) | `#[region]`, parallel states, evaluation order, entering a region state. |
| [Region coordination](./14-region-coordination.md) | `in_state` between regions, `#[unhandled]` in regions, completion of a parallel state. |
| [Timers](./15-timers.md) | `#[after]`, durations from data, stale timers and epochs, `fire`. |
| [Tasks](./16-tasks.md) | `#[invoke]`, `complete`, stale results, how a runtime feeds timers and tasks. |
| [History](./17-history.md) | `#[history]` (shallow) and `#[history(deep, keep)]`. |
| [Defer and raise](./18-defer-and-raise.md) | `defer!()`, `cx.raise`, the queues, `defer_cap`, `defer_overflow`, `raise_cap`. |
| [Testing](./19-testing.md) | The `estado-test` harness, the virtual clock and timers. |
| [Testing tasks and errors](./20-testing-tasks-and-errors.md) | Completing tasks, stale results and testing errors. |
| [Errors and safety](./21-errors-and-safety.md) | `SendError`, poisoning, livelock and `max_microsteps`, effect overflow. |
| [Warnings and options](./22-warnings-and-options.md) | W0001 to W0005, `allow(..)`, every `#[estado::machine]` option, feature detection. |
| [no_std](./23-no-std.md) | The core without `std` or `alloc`, and the hand-written API against the macro. |

## What is not available yet

Graph export (Mermaid, DOT, SCXML, JSON), the Tokio and Embassy runtimes, serde snapshots, the live inspector and the typestate mode belong to later milestones. `#[choice]`, `#[edges]`, `#[on_error]`, child machines (`#[child]`) and `#[invariant]` are planned and report error [E0090](../errors/E0090.md) today. `goto_dyn!` is planned too, but today it reports [E0040](../errors/E0040.md), and a handler that returns `Result` reports [E0061](../errors/E0061.md).
