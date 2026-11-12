# Changelog

## M1 — risk prototypes

- Typed edges `Next<M, impl Edge<M>>` work on stable; plan B for R1 is not needed (ADR 0001).
- The poisoned state lives in a core-owned `Slot<M>`, no hidden variant (ADR 0002, closes 13.1).
- Warnings W0001-W0010 use generated `#[deprecated]` items with the user's span (ADR 0003).
- Spec errata 1-4: AC-15 restated against `(M, Ctx)`, started bit, `Slot::Poisoned`, `Edge::apply`.

## M0 — skeleton

- Workspace with the nine crates from spec section 2, all empty.
- CI: rustfmt, clippy `-D warnings`, tests in debug and release (AC-19), MSRV 1.85, `estado-core` on `thumbv7em-none-eabihf` without alloc (AC-16), Miri (AC-18).
- `docs/` layout: spec, errata, ADRs, error pages.
