# Reference

Lookup pages for `estado`. Each page answers precise questions: what an attribute accepts, what a handler can take, what an error means. If you are learning the library, start with the [guide](../guide/) instead.

| Page | Contents |
| --- | --- |
| [Attributes](attributes.md) | `#[machine(..)]` and its options, `#[root]`, `#[state]`, `#[initial]`, `#[terminal]`, `#[substates]`, `#[region]`, `#[history]`, `#[after]`, `#[invoke]`, `#[unhandled]`, `#[handlers]` and the role attributes. Syntax, rules, error codes. |
| [Macros](macros.md) | `goto!` in every form, `stay!`, `bubble!`, `unhandled!`, `defer!`, `reenter!`, and the reserved `goto_dyn!` and `edges!`. |
| [Handlers](handlers.md) | The parameters a handler can take and their order, per role; return types; the generated `*Fields` and `*Mut` views. |
| [Machine](machine.md) | `Machine<M>`: `new`, `start`, `send` and its variants, `fire`, `complete`, state accessors, `Step` and `Report`, `StatePath`. |
| [Context](context.md) | `Cx`: `ctx`, `ctx_mut`, `emit`, `raise`, `in_state`; the state id constants. |
| [Effects](effects.md) | `Effect`, `Sink`, `InlineEffects`, `Driver` and `drive`; timer and task keys, epochs, and how a runtime feeds results back. |
| [Errors](errors.md) | `SendError`, poisoning, the livelock budget, and every compile-time code. |
| [Configuration](configuration.md) | Crates and their status, Cargo features, the facade and prelude, `no_std`, the Rust version. |
| [Graph](graph.md) | `estado-graph`: `to_mermaid`, `to_dot`, `to_scxml`, `to_json`, `GraphExport`, `Format`, `assert_snapshot!`, the JSON schema. |
| [Testing](testing.md) | `estado-test`: `Harness`, the virtual clock, `assert_state!`. |
| [Core](core.md) | `estado-core` without the macro: the `Spec` trait and hand-written machines. |

The pages for each compile-time code are in [`../errors/`](../errors/README.md).

## Conventions

- Examples are complete programs. They are compiled and run as tests with the rest of the documentation.
- `Root` stands for the `#[root]` enum of a machine. `Event`, `Command` and `Ctx` stand for the types given in `#[machine(event = .., effect = .., ctx = ..)]`.
- Error and warning codes (`E0006`, `W0002`) link to their own page.
