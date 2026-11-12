# 0001. Typed edges with `Next<M, impl Edge<M>>`

Status: accepted
Date: 2026-11-12
Spec: 4.1, 4.2, 6.3, risk R1

## Context

Spec 4.2 wants every handler to return `Next<Self, impl Edge<Self>>`, where the hidden type is an edge enum generated per handler and generic over the types of the constructor closures of `goto!(T, |old| ..)`. Risk R1: inference might fail in real handlers (several branches, nested matches, early return, `?`, `goto!` inside closures). Plan B was to require non-capturing constructors (`fn` pointers) and a non-generic edge enum.

The spike in `spikes/typed-edges/` hand-writes the code the macro would generate for the media player of spec 3.2 and checks it on stable Rust (edition 2024).

## Decision

Keep the technique of spec 4.2. Plan B is not needed. The generated code follows these rules:

1. **One variant per `goto!` occurrence**, not per target. Each closure has its own type, so two constructors for the same target cannot share a generic variant (`compile_fail` case 3). The graph still sees one edge per (handler, target): `Edge::TARGETS` lists one entry per variant and the graph builder deduplicates.
2. **One generic parameter per constructor occurrence.** Simple-form edges carry plain data and add no parameter. A handler without any `goto!` gets the concrete `NoEdge` type instead of `impl Edge`.
3. **Constructor parameters are annotated.** The macro emits `|old: Exiting![X]|` with the concrete type, so closures typecheck without a bound at the construction site; the `FnOnce(X) -> Target` bound lives on the generated `impl Edge`.
4. **Precise capturing is mandatory.** The rewritten return type is `Next<Self, impl Edge<Self> + use<'event, S>>`: the event lifetime (the macro names it when it rewrites the signature) plus every type parameter in scope (the sink `S`). Without `use<..>`, edition 2024 captures the borrow of the state data, and the transition cannot take the state after the handler returns (`compile_fail` case 1, E0506). `use<..>` is stable since 1.82, below the 1.85 MSRV.
5. **`Edge` gets an `apply` method.** The generated dispatch cannot name the hidden type, so the transition code (spec 5.3) lives in `Edge::apply`, monomorphized per handler. Spec 4.2 only shows `const TARGETS`; the method is `#[doc(hidden)]` and is part of what the macro generates against the public core API (principle 6).

Cases verified to infer and run correctly: several branches with different constructors, nested `match` with guards, early `return`, `?` in a handler returning `Result<Next<..>, E>`, `goto!` inside a closure passed to `Option::map`, `break` with a value out of a `loop`, constructors that borrow event data, constructors that move non-`Clone` data (`Vec`) out of the exiting state.

## Consequences

- A constructor that captures the state data (the handler's `&mut` parameters, `*Fields`, `cx`) is rejected, as it should be: that data is about to be taken. The compiler error is E0700 ("hidden type captures lifetime that does not appear in bounds") and its `help` suggests adding `'_` to `use<..>`, which the user cannot do in macro-generated code and which would be wrong anyway. This is a case of risk R5. Mitigation: the macro checks constructor closures for uses of the handler's state parameters and emits its own error with a fix ("copy the value into a local before the `goto!`"). Until then, `docs/errors/` documents E0700 next to the macro errors.
- Precise capturing means constructors may borrow only from the event. Data from the state must be copied or moved into locals first, or taken from the `old` argument.
- The number of generic parameters grows with the number of constructor `goto!`s in one handler. This has no runtime cost; compile time is watched by BM-05.
- The spike's `Next<E>` drops the `M` parameter for brevity; the real `Next<M, E>` keeps it as specified.
