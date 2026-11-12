# 0003. Warnings from proc-macros via deprecated items

Status: accepted
Date: 2026-11-12
Spec: 6.7
AC-17

## Context

Spec 6.7 defines warnings W0001-W0010. Stable proc-macros cannot emit warnings
(`proc_macro::Diagnostic` is nightly-only, and nightly is forbidden). A spike in
`spikes/warnings/` (edition 2024, tested on stable 1.96 and 1.99; 1.85 is not
installed in the spike environment, but the technique only uses `#[deprecated]`
and `quote_spanned!`, stable since Rust 1.x) verified the technique required by
the spec and compared variants.

Alternatives rejected:

- `#[must_use]` on a fn or struct: works, but the message is only a `= note:`
  and rustc appends a misleading `help: use let _ = ... to ignore the resulting
  value`. Struct form also adds a `path_statements` warning.
- `proc-macro-warning` crate: the same deprecated-item technique behind an
  extra dependency. Not needed.
- `#[diagnostic::on_unimplemented]`: only decorates trait-bound errors, cannot
  produce a warning.
- `proc_macro::Diagnostic`: unstable, nightly only. Recorded for reference.
- Fake `unused_variables`/`non_snake_case` bindings: message cannot be controlled.

## Decision

Each warning is two generated pieces:

1. A `#[deprecated(note = "estado::W0001: ...")]` item inside the existing
   `#[doc(hidden)] mod __estado_<machine>`, one item per occurrence, with a
   hash-free name such as `W0001_Orphan`.
2. A reference to it in an anonymous `const _: () = { ... };` next to the user's
   item, written with `quote_spanned!` using the span of the offending
   identifier (for W0001, the variant ident). The path ident must carry that
   span too.

Chosen kind: `const fn` called as `__estado_Player::W0001_Orphan();`. A deprecated
`const` triggers `clippy::ignored_unit_patterns` under pedantic (`let _ = ()`);
a deprecated unit struct works equally but reads "unit struct". `fn` is clean
under clippy pedantic and nursery.

`#[machine(allow(W0001))]` suppresses emission of that code entirely (nothing is
generated), so it does not depend on lint levels. Unknown codes are an error at
the code's span. The note always ends with
``(silence with `#[machine(allow(W0001))]`)``.

Exact output (default lint level, 1.99):

```text
warning: use of deprecated function `__estado_Player::W0001_Orphan`: estado::W0001: state `Orphan` is unreachable from the initial state (silence with `#[machine(allow(W0001))]`)
 --> machine-demo/examples/probe.rs:9:5
  |
9 |     Orphan,
  |     ^^^^^^
  |
  = note: `#[warn(deprecated)]` on by default
```

Test strategy for AC-17 (one snapshot per W code):

- **Emits**: `tests/ui/wNNNN_emits.rs` starts with `#![deny(deprecated)]`, so the
  warning becomes a deterministic error and `trybuild`'s `compile_fail` snapshots
  the `.stderr` (it ends with `note: the lint level is defined here`).
- **Allow silences**: `tests/ui/wNNNN_allow_silences.rs`, same fixture with
  `#[machine(allow(WNNNN))]`, run with `trybuild`'s `pass` under the same
  `deny(deprecated)`.
- **Warn level**: one integration test runs `cargo check --message-format=json`
  on an example and asserts level `warning`, count, line and column. It also
  asserts silence under `--cap-lints=allow`.

## Consequences

- Works on stable: precise span on the identifier, one warning per occurrence,
  works inside fn bodies and modules, `-D warnings` and `-D deprecated` turn it
  into an error, `cargo check` and `cargo build` both show it.
- The span MUST be a user span (the input tokens). With `Span::call_site()` or
  `mixed_site()` the warning still appears but points at the whole
  `#[machine(...)]` attribute and adds a "originates in the attribute macro"
  note, which is imprecise. Never emit warnings with a synthetic span.
- Hidden in dependencies: cargo applies `--cap-lints allow` to non-workspace
  crates, so warnings from a dependency's machines are not shown (intended).
  Workspace/path crates still show them.
- Lint name is `deprecated`. The header says "use of deprecated function ...",
  which is rustc's wording and cannot be changed; the code (`estado::W0001`) and
  the state name are in the note. Under `-D warnings` users see
  `-D deprecated implied by -D warnings` and `help: ... add #[allow(deprecated)]`;
  the docs for each W page (`docs/errors/`) must say to prefer
  `#[machine(allow(WNNNN))]`.
- A user `#![allow(deprecated)]` on the crate or on an enclosing module hides all
  W codes. `#[allow(deprecated)]` on the enum itself does NOT (the `const _` is a
  sibling). Acceptable and documented.
- Snapshot wording is rustc-version dependent. Spike output was identical on 1.96
  and 1.99. Snapshots are pinned to the stable toolchain used in CI.
- `trybuild` 1.0.120 requires Rust 1.88, above the 1.85 MSRV. It is a
  dev-dependency only, so the tests run on a newer toolchain in CI while the
  libraries keep MSRV 1.85.
- Open item: rust-analyzer could not be run in the spike. It may render the
  variant ident with a deprecated strikethrough; verify manually in an editor.
  Completions are not polluted: the module is `#[doc(hidden)]` and the usage is
  an unnamed `const _`.
