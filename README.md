# estado

Hierarchical statecharts for Rust: handlers in idiomatic Rust, an exact static graph and an optional typestate mode, at zero cost when a feature is not used.

> Status: v0 in development. The normative specification is [`docs/spec.md`](docs/spec.md).

## Crates

| Crate | Contents |
| --- | --- |
| `estado-core` | Traits, step algorithm, effects, graph types (`no_std`, no alloc) |
| `estado-macros` | `#[machine]`, `#[state]`, `#[handlers]`, `goto!` |
| `estado` | Facade |
| `estado-graph` | Mermaid/DOT/SCXML/JSON export and analyses |
| `estado-test` | Harness with a virtual clock, model-based testing |
| `estado-serde` | Versioned snapshot and restore |
| `estado-tokio` / `estado-embassy` | Runtimes |
| `estado-inspect` | Live inspector and replay |

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
