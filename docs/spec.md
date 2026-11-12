# estado — v0 implementation specification

2026-11-12 · Diogo Leite

## 0. How to use this document

This document is the normative specification of v0 of `estado`, a hierarchical state machine library for Rust. It was written for an implementing agent: everything here is a requirement, except where marked **[OPEN]** (section 13).

**Normative language** (RFC 2119):

- **MUST**: mandatory. Violating it is a bug.
- **SHOULD**: mandatory unless there is a technical reason documented in the PR.
- **MAY**: optional.

**Recommended reading order:** 1 → 3 → 5 (semantics) → 4 (core) → 12 (plan) → the rest as needed per milestone.

**Rules for the implementer:**

1. Implement in the order of the milestones in section 12. Do not start the macro (M3) before the reference scenarios run, hand-written, on top of the core (M2).
2. Section 5 (semantics) is the source of truth. If code in another section contradicts section 5, section 5 wins and the contradiction must be recorded in `docs/spec-errata.md`.
3. The code examples are **illustrative of the public API**. Names of internal types, module layout and private signatures are your decision.
4. Every decision made on an [OPEN] item must be recorded in `docs/decisions/NNNN-title.md` (short ADR format: context, decision, consequences).
5. Each acceptance criterion in section 10 must become an automated test in CI.
6. When in doubt between more features and simpler, choose simpler and open an item in `docs/decisions/`.

## 1. Overview

`estado` is a statechart library for Rust that combines three things that no library delivers together today: handlers in idiomatic Rust (like `statig`), an exact static graph (like the `rust-fsm`/`smlang` DSLs) and an optional typestate mode (like `statum`). All of this at zero cost when the features are not used.

### 1.1 Principles, in priority order

When two conflict, the one above wins.

1. **Semantic correctness before sugar.** Every behavior has a rule written in section 5.
2. **True zero cost.** The minimal mode MUST match a hand-written `enum` + `match` (5% tolerance).
3. **The graph is the truth.** The exported graph MUST correspond exactly to the possible transitions, guaranteed by the compiler.
4. **Idiomatic Rust.** Ownership, `match`, `?`, rust-analyzer and rustfmt work without friction. No custom DSL and no parameters injected by name.
5. **Sans-IO, deterministic core.** The core has no clock, async, IO or mandatory allocation.
6. **The macro is sugar, not lock-in.** All generated code MUST be writable by hand against the public API of `estado-core`.

### 1.2 Guarantees

- **Determinism:** the same sequence of events and the same initial state produce the same sequence of effects and states.
- **No UB:** no `unsafe` in the public API. Internal `unsafe` MAY exist in the core only with a `// SAFETY:` comment and a Miri test.
- **No half-built state:** a panic during a transition leaves the machine in `Poisoned` (section 3.6).
- **Exact graph:** all edges appear in the graph; dynamic edges (`goto_dyn!`) are marked as such.
- **Stale events are harmless:** timer/invoke events from a state instance that has already exited are discarded (section 7).

### 1.3 Non-goals of v0

- Durable execution engine (Temporal, Restate). The library only offers snapshot/restore.
- Full SCXML conflict-resolution semantics for parallel regions.
- Visual editor with round-trip to code.
- Parallel regions in typestate mode.
- Struct-of-arrays for many instances.
- Exactly-once guarantee for effects.

## 2. Crate architecture

The project is a Cargo workspace with small crates. `estado-core` is the only mandatory one and MUST compile as `no_std` without `alloc`.

| Crate | Contents | `no_std` | Depends on |
| --- | --- | --- | --- |
| `estado-core` | Traits, step algorithm, `Next`, `Cx`, `Effect`, effect sinks, `Graph` (types), errors | Yes, without alloc | nothing (optional: `heapless`, `smallvec`) |
| `estado-macros` | `#[machine]`, `#[state]`, `#[handlers]`, `goto!` and the rest | n/a (proc-macro) | `syn` 2, `quote`, `proc-macro2` |
| `estado` | Facade: re-exports core + macros, `prelude` | Yes | core, macros |
| `estado-graph` | Mermaid / DOT / SCXML / JSON export, analyses (reachability, paths) | Yes, with alloc | core |
| `estado-test` | `Harness` with virtual clock, model-based testing, `proptest` strategies | No | core, graph, `proptest` |
| `estado-serde` | Versioned snapshot/restore, migrations, `estado.lock.json` | Yes, with alloc | core, `serde` |
| `estado-tokio` | Actor, timers, invokes with cancellation, `subscribe()` | No | core, `tokio` |
| `estado-embassy` | Timers and actor for embedded | Yes, without alloc | core, `embassy-time`, `embassy-sync` |
| `estado-inspect` | Live inspector (WebSocket JSON), trace recording and replay | No | core, graph |

### 2.1 Features of `estado` / `estado-core`

| Feature | Default | Effect |
| --- | --- | --- |
| `std` | yes | Implements `std::error::Error`, enables `alloc` |
| `alloc` | via `std` | `Vec<Machine<_>>` of children, spill of the effect buffer to the heap, `goto_dyn!` with an allocated path |
| `tracing` | no | Span per event, event per transition |
| `inspect` | no | `Inspector` hook called on every microstep |
| `invariants` | no | Checks `#[invariant]` in release; in debug it always checks |
| `serde` | no | `Serialize`/`Deserialize` for the snapshot types |

### 2.2 General constraints

- **MSRV:** stable Rust 1.85 (edition 2024). Requires native async fn in traits and `impl Trait` in trait return position.
- **Nightly:** forbidden in any crate.
- **Core dependencies:** at most two, all optional.
- **License:** MIT OR Apache-2.0.
- **Repository layout:** `crates/<name>`, `examples/` (one per scenario in section 11), `benches/`, `docs/` (spec, ADRs, errata, explanation of each error code).

## 3. Data model

The state hierarchy is represented by **nesting of Rust types**: a superstate is a struct that holds its shared data and an enum with the substates. This gives correct ownership, native exhaustive `match` and a predictable `size_of`.

### 3.1 Vocabulary

| Term | Definition |
| --- | --- |
| **Machine** | The root type annotated with `#[machine]`. Always an `enum`. |
| **Atomic state (leaf)** | Variant with no substates or regions. Unit, tuple or struct-like. |
| **Compound state** | Variant whose payload is a `#[state]` struct with exactly one `#[substates]` field. |
| **Parallel state** | Variant whose payload is a `#[state]` struct with two or more `#[region]` fields. |
| **Active configuration** | The set of active states: the root → leaf path, with one branch per region. |
| **LCA** | Lowest common ancestor of the source and the target of a transition. |
| **Exit segment** | The states that exit in a transition: from the leaf up to the child of the LCA (inclusive). |
| **State instance** | A concrete entry into a state. Re-entering creates a new instance, with a new epoch. |
| **Epoch** | A `u32` counter per state that has timers or invokes, incremented on every entry (wrapping). |

### 3.2 Shape of states

```rust
#[machine(event = Ev, effect = Cmd, ctx = Env)]
enum Player {
    #[initial] Idle,
    Active(Active),                 // compound
    Failed { reason: String },      // atomic with data
}

#[state]
struct Active {
    track: TrackId,                 // superstate data
    buf: AudioBuffer,               // non-Copy: lives as long as Active exists
    #[substates] sub: ActiveSub,
}

#[state]
enum ActiveSub {
    #[initial] Playing { pos: Duration },
    Paused { pos: Duration },
}
```

Rules:

- Every state `enum` (root or `#[substates]`) MUST have exactly one `#[initial]` variant, unless no `goto!` enters the compound state without an explicit path. In that case the macro MUST emit `E0010` only if there is a use that needs the initial.
- Maximum depth: 16 levels (const `MAX_DEPTH`). Above that, error `E0011`.
- State names MUST be unique per machine, across all levels. A duplicate yields `E0012`, because `goto!(Name)` must be unambiguous.
- A variant MAY be marked `#[final]`, with an optional payload as output (`Done(Receipt)`). A final state cannot have a handler that transitions out.

### 3.3 Parallel regions

```rust
#[state]
struct Connected {
    session: Session,
    #[region] link: Link,
    #[region] keepalive: Keepalive,
}
```

Each `#[region]` field is a state enum with its own `#[initial]`. The declaration order defines the evaluation order (section 5.6). A single struct MUST NOT have `#[substates]` and `#[region]` together.

### 3.4 History

- `#[history]` on the compound struct: **shallow**. It stores only the discriminant (`u8`) of the last direct substate. The substate's data is rebuilt by `#[history_init]` or by the transition's constructor.
- `#[history(deep, keep)]`: stores the full value of the substate enum in an `Option<ActiveSub>` kept **in the parent of the compound state** (or at the root). The memory cost is visible and is included in the size warning.
- History is written on exit from the compound state and read on `goto!(X, history)`. With no previous record, history uses the `#[initial]`.

### 3.5 Runtime representation

`Machine<M>` MUST have this logical content, and nothing more when the optional features are not used:

```rust
pub struct Machine<M: Spec> {
    state: M,                         // the root enum, including nested data
    ctx: M::Ctx,                      // global context (ZST if not declared)
    rt: M::Runtime,                   // ZST when there are no epochs, defer, history, raise
}
```

`M::Runtime` is generated by the macro and contains only what is used: epochs of the states with timer/invoke, history discriminants, the defer queue (`heapless::Deque<E, N>` or `VecDeque` with alloc) and the internal `raise` queue.

**Size requirement:** with no optional feature, `size_of::<Machine<M>>() == size_of::<M>() + size_of::<M::Ctx>()`. Covered by a test.

### 3.6 Poisoning

The macro MUST add to the root enum a hidden variant `#[doc(hidden)] __Poisoned`. During the construction phase of a transition (section 5.3, steps 4–6), the state is `__Poisoned`. If a panic occurs in that interval, the machine remains `__Poisoned`, and every later `send` MUST return `Err(SendError::Poisoned)`.

- `m.state()` on a poisoned machine MUST panic with a clear message. `m.try_state()` returns `Option<&M>`.
- `__Poisoned` MUST NOT appear in the graph, in the snapshot or in the user's `match` expressions. How to hide the variant is open (13.1): (a) the macro generates an internal mirror enum `__Repr` with the extra variant and exposes `M` without it; or (b) it stores `Option<M>` and uses `take()`. The selection criterion is the smallest `size_of` and the simplest generated code. Both MUST respect the size requirement of 3.5 when the enum has a niche available.

## 4. `estado-core` API

The core exposes a `Spec` trait, which the macro implements (and which can be implemented by hand), and a `Machine<M>` type that runs the algorithm of section 5. The signatures below are the public contract; details of lifetimes and bounds MAY be adjusted if the behavior is kept.

### 4.1 `Spec` trait

```rust
pub trait Spec: Sized {
    type Event;
    type Effect;
    type Ctx;
    type Output;                            // payload of the root #[final], or ()
    /// Auxiliary state (epochs, history, queues). ZST when nothing is used.
    type Runtime: Default;

    const NAME: &'static str;
    const GRAPH: &'static Graph;

    fn initial() -> Self;
    fn state_id(&self) -> StateId;          // id of the active leaf (or of the parallel state)
    fn path(&self) -> StatePath;            // full active path, without allocating

    /// Initial entry: entries from the root to the initial leaf.
    fn enter_initial<S: Sink<Self>>(&mut self, cx: &mut Cx<'_, Self, S>);

    /// One complete microstep: decision (5.1) and, if there is a transition, execution (5.3).
    fn microstep<S: Sink<Self>>(&mut self, cx: &mut Cx<'_, Self, S>, ev: &Self::Event) -> Outcome;

    /// Evaluates the pending #[always]s (5.9).
    fn run_always<S: Sink<Self>>(&mut self, cx: &mut Cx<'_, Self, S>) -> Outcome;
}

pub enum Outcome { Stayed, Unhandled, Deferred, Transitioned { changed: bool } }
```

**Why decision and execution are in the same method:** the constructor form of `goto!` produces a closure of anonymous type, which cannot be stored in a named enum. The generated code calls the handler, lets the handler's borrows end, and applies the transition (5.3) in the same frame, monomorphized. This also eliminates any indirection.

### 4.2 `Next` and typed edges

```rust
/// Return value of a handler. `E` = set of edges of this handler (generated type).
pub enum Next<M: Spec, E: Edge<M> = NoEdge> {
    Stay,        // handled, no transition
    Bubble,      // delegates to the parent
    Unhandled,   // nobody handles it at this level; applies the policy (5.7)
    Defer,       // defers the event (5.5)
    Reenter,     // exit + entry of the state itself
    Goto(E),     // transition to a typed target
}

/// Implemented by the generated code for each set of edges.
pub trait Edge<M: Spec> { const TARGETS: &'static [StateId]; }
```

The user writes `-> Next<Self>` in the signature. The macro rewrites it to `-> Next<Self, impl Edge<Self>>`, where the hidden type is an enum generated per handler, generic over the types of the constructor closures. Since all `return`s of a function are unified with the same return type, inference resolves each generic parameter from the branch that builds that edge. The implementer MUST validate this technique in a prototype in M1 (risk R1, section 13).

Users MUST NOT construct `Next::Goto` directly. The macros `goto!`, `stay!` and the rest (section 6.3) are the interface.

### 4.3 `Cx`: the handler context

```rust
impl<'a, M: Spec, S: Sink<M>> Cx<'a, M, S> {
    pub fn ctx(&self) -> &M::Ctx;
    pub fn ctx_mut(&mut self) -> &mut M::Ctx;
    pub fn emit(&mut self, cmd: M::Effect);                 // user effect
    pub fn raise(&mut self, ev: M::Event);                  // internal event (5.4)
    pub fn in_state(&self, id: StateId) -> bool;            // read-only query, including sibling regions
    pub fn child<'c, C: Spec>(&'c mut self, m: &'c mut Machine<C>) -> ChildCx<'c, M, C, S>
        where M::Effect: From<C::Effect>;                   // section 7.4
}
```

`in_state` MUST work without accessing the data of other regions (the borrow checker does not allow it). Suggested implementation: a snapshot of the active discriminants (`[u8; N]`), updated at the start of each microstep.

### 4.4 Effects and sinks

```rust
pub enum Effect<C> {
    User(C),
    StartTimer { key: TimerKey, after: Duration },
    CancelTimer(TimerKey),
    Invoke { key: InvokeKey, task: C },
    Cancel(InvokeKey),
}

pub trait Sink<M: Spec> {
    fn push(&mut self, e: Effect<M::Effect>) -> Result<(), Overflow>;
}
```

Required `Sink` implementations: `Vec` (alloc), `SmallVec` (feature), `heapless::Vec`, any `FnMut(Effect<C>)`, and `Discard` (an explicit type that discards everything; used with `effect = ()`).

`Duration` is `core::time::Duration`. `TimerKey` and `InvokeKey` contain: the owning `StateId`, the epoch and a `ChildPath` (section 7.4). Both are `Copy`, `Eq`, `Hash` and serializable.

### 4.5 `Machine<M>`

```rust
impl<M: Spec> Machine<M> {
    pub const fn new(ctx: M::Ctx) -> Self;                  // does not run entries; const when possible
    pub fn start(&mut self) -> Result<Step<M>, SendError<M::Event>>; // runs enter_initial

    pub fn send(&mut self, ev: M::Event) -> Result<Step<M>, SendError<M::Event>>;
    pub fn send_into<S: Sink<M>>(&mut self, ev: M::Event, sink: &mut S) -> Result<Report, SendError<M::Event>>;
    pub fn send_with(&mut self, ev: M::Event, f: impl FnMut(Effect<M::Effect>)) -> Result<Report, SendError<M::Event>>;
    pub fn send_batch<S: Sink<M>>(&mut self, evs: impl IntoIterator<Item = M::Event>, sink: &mut S) -> Result<Report, SendError<M::Event>>;

    pub fn state(&self) -> &M;            // panics if poisoned
    pub fn try_state(&self) -> Option<&M>;
    pub fn ctx(&self) -> &M::Ctx;
    pub fn ctx_mut(&mut self) -> &mut M::Ctx;
    pub fn is_in(&self, id: StateId) -> bool;
    pub fn path(&self) -> StatePath;      // Display: "Connected/link:Open,keepalive:Pinging"
    pub fn is_done(&self) -> bool;
    pub fn take_output(&mut self) -> Option<M::Output>; // when in a #[final] with payload
    pub fn is_poisoned(&self) -> bool;
}
```

Rules:

- `send` on a machine that has not been started MUST implicitly run `start` first, and the effects of the initial entry go into the same `Step`.
- `Step<M>` contains the effects in an inline buffer of capacity `N` (default 4, configurable in `#[machine(fx_inline = N)]`) plus a `Report`. Without `alloc`, overflow of the inline buffer returns `Err(EffectOverflow)`; with `alloc`, it spills to the heap.
- `Report` contains: `handled: bool`, `transitions: u16`, `microsteps: u16`, `ignored: Option<IgnoreReason>` (`Unhandled` or `Stale`).

### 4.6 Errors

```rust
pub enum SendError<E> {
    Poisoned,
    Unhandled(E),              // policy = error; returns the event to the caller
    DeferOverflow(E),          // defer queue full with policy = error
    RaiseOverflow,
    EffectOverflow,
    Livelock { microsteps: u16, last: StatePath },
}
```

A stale event is NOT an error: it returns `Ok` with `ignored = Some(Stale)`. All error types MUST implement `Debug`, `Display` and, with `std`, `std::error::Error`.

## 5. Normative semantics

This section is the source of truth. A `send` is **run-to-completion**: it processes the external event and all its consequences (internal events, redelivered deferred events, automatic transitions) before returning. Each processing of an event is a **microstep**.

### 5.1 Decision phase (dispatch)

1. It starts at the active leaf. If the state has a handler, it is called; if it does not, that is equivalent to `bubble!()`.
2. `bubble!()` passes the event to the parent's handler. At the root, `bubble!()` is equivalent to `unhandled!()`.
3. In a parallel state, dispatch follows section 5.6.
4. Policy override: if a state `S` has `#[unhandled(p)]` and the event leaves `S` unhandled (`unhandled!()` or `bubble!()` from `S`), `p` is applied there, without going further up.
5. Handlers receive `&mut` to the data of the state and of the parent (split borrow, section 6.2). Mutations made in the decision phase hold even if the result is `unhandled!()`. This is documented as a rule: handlers SHOULD mutate only when they return `stay!()` or `goto!`.

### 5.2 Transition kinds

Let `L` be the active leaf, `H` the state whose handler decided and `T` the target.

| Form | LCA | Exits | Enters |
| --- | --- | --- | --- |
| `goto!(T)`, `T` outside the subtree of `H` | `lca(H, T)` | `L` up to the child of the LCA | from the child of the LCA down to `T`, then the initials of `T` |
| `goto!(T)`, `T` a strict descendant of `H` (local) | `H` | `L` up to the child of `H` | from the child of `H` down to `T`, then the initials |
| `goto!(T)`, `T` a strict ancestor of `H` (local) | `T` | `L` up to the child of `T` | initial (or history) of `T` |
| `goto!(T, external)`, `T` an ancestor of or equal to `H` | `parent(T)` | `L` up to `T` (inclusive) | `T` and its initials |
| `goto!(H)` or `reenter!()` | `parent(H)` | `L` up to `H` (inclusive) | `H` and its initials |
| `stay!()` | n/a | nothing | nothing |

Additional rules:

- A transition between sibling regions of the same parallel state is forbidden (`E0021`). Use `cx.raise`.
- A transition from a region to outside the parallel state exits all regions.
- Entering a compound state without an explicit substate enters the `#[initial]`. Entering a parallel state enters all regions, each in its initial.
- `goto!(T, history)` enters the substate recorded by the history of `T`; with no record, the initial.

### 5.3 Transition execution phase

Normative order, no exceptions:

1. **Exits**, from the deepest upward, up to the child of the LCA. In a parallel state: regions in **reverse** declaration order, each from the deepest upward, then the parallel state itself. Each exit receives `&mut` to the state's data.
2. **Cancellations:** for each state that exited and has `#[after]` or `#[invoke]`, emits `CancelTimer` / `Cancel` with the key of the instance.
3. **History:** records the discriminant (or the value, if `deep, keep`) of the compound states with `#[history]` that exited.
4. **Take:** the value of the child of the LCA is taken by value (the machine is `__Poisoned` during this interval).
5. **Constructor:** receives the exit segment by value and returns the new value of the child of the LCA. The constructor's code area is the place for **transition actions** in the UML sense: it runs after the exits and before the entries.
6. **Write:** the new value is written; the machine is no longer poisoned.
7. **Entries**, from the child of the LCA downward to the leaf (initials and history resolved). In a parallel state: the state, then the regions in **declaration order**. Each entry receives `&mut` to the data.
8. **Epochs:** each state that entered and has timers or invokes gets `epoch += 1` (wrapping), before emitting.
9. **Scheduling:** emits `StartTimer` / `Invoke` for the states that entered.
10. **Always:** if the new leaf (or any state that entered) has `#[always]`, schedules an evaluation as the next microstep (section 5.9).

Entry and exit MUST NOT transition. They MAY call `cx.emit` and `cx.raise`. They return no value.

### 5.4 Queues and processing order

There are three queues, besides the external event:

- **Internal** (`cx.raise`): FIFO, capacity `raise_cap` (default 8). Overflow: `Err(RaiseOverflow)` and the machine is poisoned (it is a modeling bug).
- **Defer**: FIFO, capacity `defer_cap` (default 8), persists across `send`s.
- **Requeue**: temporary. It receives the entire content of the defer queue when the active configuration changes.

Normative algorithm of `send(e)`:

```
send(e):
  0. if poisoned → Err(Poisoned)
  1. if not started → start() (effects go to the same sink)
  2. microstep(e)                      // the external event is always first
       if Unhandled and policy = error → Err(Unhandled(e)), nothing changed
  3. loop:
       ev ← next of: pending always > internal > requeue
       if none → exit
       microstep(ev)                   // Unhandled here never becomes Err: it is counted in Report
       if microsteps > MAX (default 64) → poison, Err(Livelock)
  4. Ok(Report)

microstep(ev):
  r ← dispatch(ev)
  Stay      → nothing
  Unhandled → apply policy (5.7)
  Defer     → defer.push(ev) (overflow: defer_overflow policy)
  Transition(t) → apply(t) (5.3); if the configuration changed: requeue.extend(defer.drain())
  check the #[invariant]s; call the inspector
```

The order **always > internal > requeue** is normative: the immediate consequences of the transition come before the deferred events.

### 5.5 Defer

- `defer!()` requires ownership of the event. Machines whose `Event` has a lifetime cannot use `defer!()` (`E0030`).
- A deferred event is redelivered on every configuration change and can be deferred again.
- Overflow policy (`#[machine(defer_overflow = drop_oldest | error)]`, default `error`): `error` returns `Err(DeferOverflow(ev))` without changing state.
- The defer queue goes into the snapshot (section 8.4).

### 5.6 Parallel states

For an event that arrives at a parallel state `P` (coming from below or with `P` on the active path):

1. The regions are evaluated **in declaration order**. Each region dispatches from its leaf up to the root of the region.
2. If the region returns a transition that is **internal to the region**, it is applied immediately (5.3), before the next region.
3. If the region returns a transition **out of `P`**, it is applied and the remaining regions are NOT evaluated (short-circuit).
4. The event is **consumed** if at least one region returned `stay!`, `goto!` or `reenter!`.
5. If no region consumed it and at least one returned `defer!`, the event is deferred. If none consumed or deferred it, the event goes to the handler of `P` (bubble).
6. `P` is complete (`done`) when all regions are in a `#[final]` state.

This semantics is simpler than that of SCXML, by conscious decision. An alternative (all regions see the event before any is applied) is open (13.4).

### 5.7 Unhandled event policies

| Policy | External event | Internal event/requeue |
| --- | --- | --- |
| `ignore` (default) | `Ok`, `ignored = Unhandled` | counted in `Report`, continues |
| `error` | `Err(Unhandled(ev))`, nothing changed | counted in `Report`, continues |
| `panic` | panic | panic |

The behavior MUST NOT vary between debug and release. Logging in debug is allowed (feature `tracing`).

### 5.8 `#[choice]` pseudostates

A `#[choice] fn` is a valid target of `goto!`. It is evaluated **in the decision phase**: the result of `goto!(Choice)` is the return value of the choice function, which MUST be a `goto!` to a real state or to another choice. In this way the transition is fully resolved before execution. Choice chains have a maximum depth of 4; cycles are detected statically (`E0031`).

### 5.9 `#[always]` automatic transitions

A state with `#[always] fn(&mut S, &mut Cx) -> Next` has that function evaluated as a microstep right after entering (step 10 of 5.3). `stay!()` ends it; `goto!` transitions. It counts toward `MAX` (livelock protection).

### 5.10 Final states and `done`

- When the active substate of a compound state `C` enters a `#[final]`, the core performs a synthetic microstep calling `#[on_done(C)] fn(&mut CData, &Output, &mut Cx) -> Next`. Without `on_done`, nothing happens.
- At the root: entering a `#[final]` makes `is_done()` true. Every later `send` returns `Ok` with `ignored = Unhandled`, regardless of the policy.

### 5.11 Panics

A panic at any point of a `send` (handler, entry, exit, constructor, invariant) MUST leave the machine poisoned. Implementation: a drop guard that, on detecting unwinding, writes `__Poisoned`. With `panic = "abort"` the guard is eliminated by the compiler.

### 5.12 Determinism

The core MUST NOT use `HashMap`/`HashSet` with iteration, clocks, randomness or global state. All iteration order is declaration order.

## 6. `estado-macros`

The macro transforms ordinary Rust code (enums, structs, an `impl`) into an implementation of `Spec`. The association between handler and state is **explicit by attribute**, never by function name, and parameters are recognized **by type**, never by name.

### 6.1 Attributes

| Attribute | Where | Purpose |
| --- | --- | --- |
| `#[machine(event = E, effect = C, ctx = X, ...)]` | root enum | Declares the machine. `effect` and `ctx` are optional (default `()`) |
| `#[machine]` options | | `unhandled`, `defer_cap`, `defer_overflow`, `raise_cap`, `fx_inline`, `max_microsteps`, `size_warn`, `typestate`, `infallible`, `version`, `allow(W....)` |
| `#[state]` | struct or enum | Marks a type as part of the machine |
| `#[initial]`, `#[initial(with = path)]` | variant | Initial state of the level; `with` points to `fn(&mut Cx, &ParentFields) -> Self` when the variant has data |
| `#[final]` | variant | Final state; optional payload becomes `Output` |
| `#[substates]` | field | Enum of the substates of a compound state |
| `#[region]` | field | A region of a parallel state |
| `#[history]`, `#[history(deep, keep)]` | compound struct | Section 3.4 |
| `#[after(dur => ev)]` | variant or struct | Timer of the state. `dur` and `ev` can be closures `\|s\| ...` over the state's data |
| `#[invoke(task)]` | variant or struct | Task of the state: `task` produces a user `Effect` (section 7.2) |
| `#[unhandled(policy)]` | variant or struct | Policy override (5.1, item 4) |
| `#[handlers]` | `impl M` | Block with handlers, entries, exits, choices |
| `#[on(S)]` | fn | Event handler of state `S` (leaf, compound, parallel or root of a region) |
| `#[entry(S)]`, `#[exit(S)]` | fn | Entry and exit actions |
| `#[choice]` | fn | Choice pseudostate (5.8) |
| `#[always(S)]` | fn | Automatic transition (5.9) |
| `#[on_done(S)]` | fn | Reaction to the final substate of `S` (5.10) |
| `#[on_child_done(field)]` | fn | Reaction to the end of a child machine (7.4) |
| `#[on_resume(S)]` | fn | Called on restore of a snapshot (8.4) |
| `#[invariant]` | fn | `fn(&M, &Ctx) -> bool`, checked after each microstep |
| `#[edges(A \| B)]` | helper fn | Declares the edges of a helper outside the handler |
| `#[on_error(S)]` | `#[handlers]` or fn | Target for the `Err` of handlers that return `Result` (6.4) |
| `#[child]`, `#[children]` | field | Child machines (7.4) |

Each state MAY have at most one `#[on]`, one `#[entry]`, one `#[exit]` and one `#[always]` (`E0005`).

### 6.2 Handler signatures

Parameters are recognized by type, all optional, **in this order**:

```rust
#[on(Playing)]
fn playing(
    s: &mut Playing,            // data of the state itself (& or &mut)
    up: ActiveFields<'_>,       // fields of the ancestor Active, without #[substates]/#[region]
    cx: &mut Cx<Self>,          // context
    ev: &Ev,                    // the event
) -> Next<Self> { ... }
```

Rules:

- **State data.** A state with data SHOULD be a newtype variant (`Playing(Playing)`), and the parameter is `&mut Playing`. For struct-like variants (`Playing { pos }`), the macro generates a view `PlayingMut<'a> { pub pos: &'a mut Duration }`.
- **Ancestor fields.** For each `#[state]` struct with `#[substates]` or `#[region]`, the macro generates `<Name>Fields<'a>` with `&'a mut` on each field that is not a substate/region. A handler MAY ask for the `Fields` of any ancestor (from the closest to the farthest). This enables the split borrow between parent and child.
- **Entry/exit:** same rule, without `ev`, returning `()`.
- **Choice:** receives `&Cx` and optionally `&` to the data of the source state. Returns `Next<Self>`.
- **Async:** handlers MUST NOT be `async` (`E0060`). Asynchronous work is done via `#[invoke]` (section 7).

### 6.3 Transition macros

| Macro | Expands to |
| --- | --- |
| `goto!(T)`, `goto!(T { .. })`, `goto!(T(x))` | Edge to `T`; value built in the decision phase (simple form) |
| `goto!(A { .. } / B { .. })` | Explicit path to a deep state; data for each intermediate level |
| `goto!(T, \|old: Exiting![X]\| expr)` | Constructor form: receives the exit segment by value (5.3, step 5) |
| `goto!(T, history)` | Enters via history |
| `goto!(T, external)` | External transition (5.2) |
| `goto!(Choice)` | Evaluates the `#[choice]` at decision |
| `goto_dyn!(expr)` | Runtime target; requires `expr: M` already constructed; LCA computed at runtime; `*` edge in the graph |
| `stay!()`, `bubble!()`, `unhandled!()`, `defer!()`, `reenter!()` | Corresponding variants of `Next` |

**Typed edges (core requirement).** `#[handlers]` rewrites the body of each function: it finds all invocations of `goto!` (including inside closures and blocks), generates an edge enum for that function and replaces each `goto!` with a variant constructor. Consequences:

- The graph (`M::GRAPH`) is the exact union of the generated edges. There is no way for a transition to exist outside the graph.
- `goto!` outside a `#[handlers]`/`#[edges]` function expands to `compile_error!` (`E0040`).
- A helper with `#[edges(A | B)]` returns `Next<Self, edges![A | B]>`; it accepts only the simple form of `goto!` (`E0041` for the constructor form), because the type must be nameable.
- The simple form with the data of an intermediate ancestor missing yields `E0020`, listing the missing fields.
- `Exiting![X]` is a type generated per edge: the owned value of the child of the LCA. For `Playing → Failed` with the LCA at the root, `Exiting![Active]` is `Active` itself (with `track`, `buf` and `sub`).

### 6.4 Errors with `?`

A handler MAY return `Result<Next<Self>, E>`. The `Err(e)` becomes a `goto!` to the state of `#[on_error(S)]`, built via `From<E> for S` (or `S: From<(E, Exiting)>` if the user wants the data that exits). The error edge appears in the graph with `kind = Error`. Without `#[on_error]` in scope, `Result` in the return type is `E0061`.

### 6.5 Generated code

- All helper code goes into a module `mod __estado_<machine>` with `#[doc(hidden)]`, and only the necessary public types (`*Fields`, `*Mut`, `StateId` consts) are re-exported.
- Dispatch MUST be a nested `match` over the discriminants. Forbidden: `Box<dyn>`, function-pointer tables, `HashMap`, allocation.
- Exit/entry sequences per edge are generated as linear code (no loop over the hierarchy). For `goto_dyn!`, a `const` table of parents and depths is used.
- User handlers get `#[inline]`. The generated code does not force `#[inline(always)]`.
- IDs: `pub const PLAYING: StateId` etc., generated as associated constants (`Player::PLAYING`, `Player::ACTIVE`).
- `cargo expand` MUST produce readable code: descriptive names, no identifiers with hashes.

### 6.6 IDE robustness

The macro MUST emit a best-effort expansion even for invalid code (errors become `compile_error!` next to the original code), so that rust-analyzer keeps completing fields and methods during editing. Never abort the whole expansion because of a local error.

### 6.7 Diagnostics

Every error has a code, a precise span, a short message and, when possible, a suggestion (`help: did you mean ...`, via edit distance). Each code MUST have a page in `docs/errors/EXXXX.md` and a `trybuild` test with a snapshot of the output.

| Code | Condition |
| --- | --- |
| E0001 | `#[machine]` outside an enum |
| E0002 | `event` not declared |
| E0003 | `goto!` target is not a state of this machine |
| E0004 | `#[on(S)]` with unknown `S` |
| E0005 | Two handlers of the same kind for the same state |
| E0006 | Handler parameter with an unrecognized type or out of order |
| E0010 | Level without `#[initial]` when an initial is needed |
| E0011 | Depth above 16 |
| E0012 | Duplicate state name |
| E0020 | Missing data for an intermediate state on the `goto!` path |
| E0021 | Transition between sibling regions |
| E0022 | Transition leaving a `#[final]` |
| E0030 | `defer!` with an event that has a lifetime |
| E0031 | Cycle among `#[choice]`s |
| E0040 | `goto!` outside `#[handlers]`/`#[edges]` |
| E0041 | Constructor form in an `#[edges]` helper |
| E0050 | Regions in typestate mode |
| E0060 | `async` handler |
| E0061 | `Result` in the return type without `#[on_error]` |

**Warnings.** Proc-macros do not emit warnings on stable Rust. The required technique is to generate a `#[deprecated(note = "estado::W0001: ...")]` item and reference it with the span of the problem. Each warning can be silenced with `#[machine(allow(W0001))]`.

| Code | Condition |
| --- | --- |
| W0001 | State unreachable from the initial |
| W0002 | State with no outgoing edges that is not `#[final]` |
| W0003 | `defer!` in a state with no outgoing edges |
| W0004 | Variant larger than `size_warn` bytes (default 128); suggests `Box` |
| W0005 | `#[after]` event with no handler that handles it (best effort) |
| W0010 | Typestate mode: `match` arm that could not be associated with an event variant |

## 7. Time, invokes, child machines and runtimes

The core knows neither time nor async. Timers and tasks belong to a **state instance**: they are scheduled on entry, cancelled on exit, and their results are only accepted if the key (state + epoch) is still the active one. Stale results are discarded by the core, regardless of whether the runtime cancelled in time.

### 7.1 Timers

```rust
#[state]
#[after(|s: &Connecting| s.timeout() => Ev::ConnectTimeout)]
struct Connecting { attempt: u8 }
```

- A state MAY have several `#[after]`s. Each has an index in the key.
- The duration is evaluated **on entry** (step 9 of 5.3), from the state's data. The event is NOT stored: the core MUST build it at firing time, evaluating the event expression with the current data of the state. This avoids storing one event per timer.
- The runtime calls `Machine::fire(key: TimerKey) -> Result<Step<M>, SendError<M::Event>>`. If the key does not correspond to the active instance (inactive state or different epoch), it returns `Ok` with `ignored = Stale`. Otherwise, it builds the event and follows the `send` algorithm (5.4).

### 7.2 Invokes

```rust
#[state]
#[invoke(|s: &Loading| Cmd::FetchUser(s.id))]
struct Loading { id: UserId }
```

- On entry, the core emits `Effect::Invoke { key, task }`. On exit, `Effect::Cancel(key)`.
- The runtime runs the task and delivers the result as an event via `Machine::complete(key: InvokeKey, ev: M::Event)`. The result → event conversion is the responsibility of the user's effect executor.
- Old key → `Ok` with `ignored = Stale`.
- The key is stable and serializable, for use as an idempotency key. The library does NOT guarantee exactly-once.

### 7.3 Runtime contract: `Driver`

Any runtime implements:

```rust
pub trait Driver<M: Spec> {
    fn effect(&mut self, cmd: M::Effect);
    fn start_timer(&mut self, key: TimerKey, after: Duration);
    fn cancel_timer(&mut self, key: TimerKey);
    fn invoke(&mut self, key: InvokeKey, task: M::Effect);
    fn cancel(&mut self, key: InvokeKey);
}
```

The core provides `estado::drive(effect, &mut impl Driver<M>)`, which dispatches each variant of `Effect`. `Driver` is synchronous: async runtimes launch their tasks from it.

### 7.4 Child machines

```rust
#[state]
struct Fulfilling {
    #[child] payment: Machine<Payment>,
    #[children] items: Vec<Machine<Item>>,    // requires alloc
}
```

- `#[child]` and `#[children]` declare fields that are machines. The parent owns the children; on leaving the state, the children are dropped and their timers/invokes are cancelled (the core emits the corresponding `Cancel`s before the drop).
- **Sending:** the handler sends to the child with `cx.child(&mut s.payment).send(ev)` or `cx.child_at(i, &mut s.items[i]).send(ev)`. The child's effects are converted and written into the parent's sink: `User(c)` becomes `User(From::from(c))`; timers and invokes gain a `ChildPath` segment (field index + index in the `Vec`).
- **Return:** `ChildStep { report, done: bool }`. When the child enters a `#[final]`, the core calls `#[on_child_done(field)] fn(&mut S, idx: usize, out: C::Output, cx) -> Next<Self>` in the same microstep.
- **Routing:** `Machine::fire`/`complete` with a non-empty `ChildPath` are routed by the generated code to the right child. If the path no longer exists (child removed, state exited), the result is `Stale`.
- `ChildPath` is `Copy`: `[u16; 4]` + length. Maximum machine nesting depth: 4.
- The child's run-to-completion happens **inside** the parent's microstep. Events raised in the child stay in the child.

### 7.5 `estado-tokio`

```rust
pub trait Effects<M: Spec>: Send + 'static {
    async fn run(&mut self, cmd: M::Effect);                      // simple effects
    async fn invoke(&mut self, task: M::Effect) -> Option<M::Event>; // state tasks
}

let h = Actor::spawn(Machine::<Conn>::new(env), MyEffects, Config { mailbox: 64 });
h.send(Ev::Connect).await?;          // enqueues
let r = h.call(Ev::Connect).await?;  // waits for processing and returns the Report
let mut rx = h.subscribe();          // stream of TransitionRecord
let snap = h.snapshot().await?;      // requires feature serde
```

Requirements:

- One task per machine. External events, timer firings and invoke results enter the **same** mailbox (bounded mpsc), guaranteeing a total order.
- Timers: `tokio::time::sleep_until`, cancelled by abort. Invokes: `tokio::spawn`, cancelled by `JoinHandle::abort`.
- `run` is awaited in order, before the next event. `invoke` runs concurrently.
- Compatible with `tokio::time::pause()` for tests.
- Mailbox full: `send` waits; `try_send` returns an error.

### 7.6 `estado-embassy`

- Without alloc: mailbox `embassy_sync::channel::Channel<_, _, N>`, timers with `embassy_time::Timer`, maximum number of simultaneous timers as a const generic.
- Invokes: executed in a user-supplied task pool, of fixed size. If the pool is full, `invoke` returns a configuration error in debug and is dropped with a log in release. **[OPEN 13.5]**: evaluate whether invokes stay out of v0 on embassy.
- `Machine::new` MUST be `const fn` to allow `static` in `Mutex<CriticalSectionRawMutex, RefCell<Machine<_>>>`.

## 8. Graph, export, analysis and persistence

The graph is a `const` value generated at compile time from the typed edges. It feeds export, static analysis, model-based tests and the inspector.

### 8.1 Structure of `Graph`

```rust
pub struct Graph {
    pub machine: &'static str,
    pub states: &'static [StateInfo],    // declaration order
    pub edges: &'static [EdgeInfo],
}
pub struct StateInfo {
    pub id: StateId, pub name: &'static str, pub parent: Option<StateId>,
    pub kind: StateKind,                 // Atomic | Compound | Parallel | Region | Final | Choice
    pub initial: Option<StateId>, pub history: HistoryKind,
    pub timers: u8, pub invokes: u8, pub depth: u8,
}
pub struct EdgeInfo {
    pub from: StateId,                   // state whose handler declares the edge
    pub to: EdgeTarget,                  // State(id) | Choice(id) | Dynamic
    pub kind: EdgeKind,                  // Normal | External | History | Error | Timer | Always | Done
    pub event_hint: Option<&'static str>,// event pattern of the match arm, when extractable
}
```

`event_hint`: when the `goto!` is inside a `match ev { Ev::Pause => ... }` arm, the macro SHOULD record the pattern (`"Ev::Pause"`) as the label of the edge. Without a clear pattern, the label is empty.

### 8.2 Export (`estado-graph`)

| Format | Function | Requirements |
| --- | --- | --- |
| Mermaid | `to_mermaid()` | `stateDiagram-v2` with compound states, `--` for regions, `[*]` for initial/final, choice as `<<choice>>` |
| DOT | `to_dot()` | Clusters for compound states; dashed dynamic edges |
| SCXML | `to_scxml()` | `<state>`, `<parallel>`, `<history>`, `<final>`; events via `event_hint`; no executable actions |
| JSON | `to_json()` | Schema documented in `docs/graph.schema.json`, used by the inspector |

Documentation test: `estado_graph::assert_snapshot!(Player::GRAPH, "docs/player.mmd")` fails if the file is out of date and rewrites it with `ESTADO_BLESS=1`.

### 8.3 Analyses

- `reachable()`: states reachable from the initial (basis of W0001).
- `dead_ends()`: non-final states with no exit (W0002).
- `edge_cover()`: minimal set of paths from the initial that covers all edges (for model-based testing).
- `shortest_path(from, to)`.
- `worst_case_actions()`: maximum number of entry+exit calls in a single transition, also exposed as `const WORST_CASE_ACTIONS: usize` for WCET analysis on embedded targets.

### 8.4 Snapshot and restore (`estado-serde`)

The snapshot contains: the schema version, the state value (nested, via serde), the context (if `Ctx: Serialize`), the history discriminants, the defer queue, the epochs, the active timers with an **absolute deadline** and the keys of the active invokes.

```rust
let snap: Snapshot<Checkout> = m.snapshot(now)?;     // `now` supplied by the caller: the core has no clock
let (m, step) = Machine::<Checkout>::restore(snap, now)?;
```

Restore semantics:

1. Rebuilds the state without running entries.
2. For each active timer, emits `StartTimer` with the remaining time (zero if it has already expired).
3. Invokes are NOT resumed automatically. For each active state with `#[invoke]`, calls `#[on_resume(S)]` if it exists. That hook decides whether to re-emit the task (with the same key, for idempotency) or to query its status. Without `on_resume`, the core re-emits `Invoke` with the same key.
4. The effects of 2 and 3 are returned in `step`.

Since the core has no clock, the absolute deadline requires the runtime to report `now` to the core at `StartTimer`. Therefore `Machine` records, per active timer, the pair (key, deadline) when the runtime confirms the scheduling via `m.timer_scheduled(key, deadline)`. Official runtimes MUST call this method; without it, the snapshot omits the timer and the restore restarts it from zero (documented).

### 8.5 Versioning and migration

- `#[machine(version = N)]` declares the schema version. Default 1.
- A generated test writes and verifies `estado.lock.json` with the shape of each version. A change of shape without incrementing `version` makes the test fail.
- Migration: `impl Migrate<2> for Checkout { fn migrate(old: v2::Snapshot) -> Result<Snapshot<Self>, MigrationError> }`. The `v2::*` types are generated from the lock by `estado_serde::versioned!("estado.lock.json", 2)`.
- `restore` accepts any version that has a chain of `Migrate` up to the current one, applying them in sequence.

### 8.6 Inspector (`estado-inspect`)

- The `Inspector` hook receives a `TransitionRecord { seq, event_name, from: StatePath, to: StatePath, edge: Option<EdgeInfo>, microstep, effects: u16 }` per microstep.
- `estado-inspect` serves the JSON graph and a stream of records via WebSocket, records to JSONL and does **deterministic replay**: reapplying the recorded events to the same initial state MUST reproduce the same records.

## 9. Typestate mode

With `#[machine(typestate)]`, the same definition generates, in addition to the dynamic mode, an API in which each state is a type parameter and invalid transitions do not compile. The two modes share handlers, graph and semantics.

### 9.1 Generated shape

```rust
#[machine(typestate, event = Ev, effect = Cmd)]
enum Conn { #[initial] Disconnected, Handshaking(Hs), Ready(Ready), Failed(Failed) }

// Generated:
pub struct ConnTs<S> { state: S, ctx: Env, rt: Runtime }   // S ∈ {Disconnected, Hs, Ready, Failed}

let c = ConnTs::new(env);                    // ConnTs<Disconnected>
let (c, fx) = c.connect(addr);               // ConnTs<Hs>
match c.handshake_reply(resp) {              // output with more than one possible target
    HsOut::Ready(c, fx)  => { /* ConnTs<Ready> */ }
    HsOut::Failed(c, fx) => { /* ConnTs<Failed> */ }
}
let dynm: Machine<Conn> = c.into();          // converts to the dynamic mode
```

### 9.2 Generation rules

- **One method per handled (state, event variant) pair.** The method name is the event variant in snake_case (`Ev::HandshakeReply(r)` → `handshake_reply(r)`). The macro discovers the pairs from the arms of `match ev` in the handler (the same mechanism as `event_hint`). Arms that it cannot associate with a variant yield `W0010` and do not become methods.
- **Return type:** if the arm has a single target, it returns `(ConnTs<Target>, Effects)`. With several targets (the set of typed edges of that arm), it returns a generated enum `<State>Out` with one variant per target. `stay!()` in an arm becomes a variant `Stayed(ConnTs<Self>)`.
- **Hierarchy:** each superstate generates a marker trait (`InActive`). Superstate handlers become methods in `impl<S: InActive> ConnTs<S>`.
- **Regions:** forbidden in typestate mode (`E0050`).
- **Timers and invokes:** available; the methods `fire(key)` and `complete(key, ev)` exist only in the dynamic mode. To use timers, convert to `Machine<Conn>`.
- **Conversion:** `From<ConnTs<S>> for Machine<Conn>` for every `S`. The inverse is `Machine::into_typed() -> ConnAny`, an enum with one variant per typed state.
- **Persistence:** `ConnAny` has the same snapshot shape as the dynamic mode.

## 10. Tests, benchmarks and acceptance criteria

v0 is only done when all the criteria in this section run in CI and pass. Each criterion has an identifier (`AC-xx`) that MUST appear in the name of the corresponding test.

### 10.1 Harness (`estado-test`)

```rust
let mut h = Harness::new(Machine::<Conn>::new(env));   // virtual clock starts at 0
h.send(Ev::Connect).expect_effect(|e| matches!(e, Cmd::Dial(_)));
h.send(Ev::Connected);
h.advance(13.secs());                                   // fires expired timers, in deadline order
assert_state!(h, Conn::Backoff(_));
h.complete_invoke(Cmd::FetchUser(_), Ev::Fetched(user)); // resolves a pending invoke
h.assert_no_pending_timers();
```

- The harness implements `Driver` in memory: it stores pending timers and invokes, and `advance(d)` fires the expired timers in deadline order (tie: scheduling order).
- `Harness::trace()` returns all `TransitionRecord`s for asserts and snapshots (`insta`).
- Model-based testing: `h.cover_edges(|path, h| { ... })` walks `GRAPH.edge_cover()`. The user supplies an event generator per edge (`EventFor`), since the graph only knows `event_hint`.
- `proptest`: `Conn::arb_events(n)` generates event sequences (requires `Ev: Arbitrary`). At each step, the `#[invariant]`s are checked.

### 10.2 Mandatory core tests

| ID | Criterion |
| --- | --- |
| AC-01 | Exit/entry order for each row of table 5.2, including local and external transitions to ancestors |
| AC-02 | Exit (reverse) and entry (forward) order of the regions; short-circuit of 5.6 |
| AC-03 | Queue order: always > internal > requeue; defer redelivered only after a configuration change |
| AC-04 | Timer/invoke event with an old epoch returns `Stale` without calling the handler |
| AC-05 | Cancellations emitted for all states that exit, including children of child machines |
| AC-06 | Panic in handler, entry, exit and constructor poisons; a later `send` returns `Poisoned` |
| AC-07 | Livelock (cycle of `always` or `raise`) poisons after `max_microsteps` |
| AC-08 | Unhandled policies (5.7) for external and internal events, with per-state override |
| AC-09 | Shallow and deep/keep history, including the case with no previous record |
| AC-10 | `on_done` in a compound state and completion of a parallel state when all regions are final |
| AC-11 | Snapshot → restore produces an equivalent machine and re-emits timers with the remaining time |
| AC-12 | Chained v1 → v2 → v3 migration |
| AC-13 | Replay of a recorded trace reproduces the same records |
| AC-14 | Generated graph equal to the expected one (snapshot) for all the examples in section 11 |
| AC-15 | `size_of::<Machine<M>>() == size_of::<M>() + size_of::<Ctx>()` for a machine without optional features |
| AC-16 | `estado-core` compiles for `thumbv7em-none-eabihf` without `alloc` |
| AC-17 | Each E and W error code has a `trybuild` test with a snapshot of the message |
| AC-18 | Miri with no errors on all core tests |
| AC-19 | Same result in debug and release for the whole suite (runs both) |

### 10.3 Benchmarks (criterion)

| ID | Benchmark | Target |
| --- | --- | --- |
| BM-01 | HTTP/1 parser (scenario A) vs. an equivalent hand-written parser, 1 MB of requests | Within 5% of the hand-written one |
| BM-02 | Flat ping-pong of 2 states, 10⁶ events, `effect = ()` | Within 5% of a hand-written `enum` + `match` |
| BM-03 | 4-level HSM, transition between cousins, entry/exit at all levels | Equal to or better than `statig` in its latest version |
| BM-04 | 10,000 machines (scenario D), 60 events each | Zero allocations per event (verified with a counting allocator) |
| BM-05 | Compile time of a synthetic machine with 200 states and 600 edges | Under 3 s of expansion + checking on a standard CI machine |

CI MUST publish the results and fail if a benchmark regresses by more than 10% against the main branch.

### 10.4 Quality

- `clippy -D warnings` and `rustfmt --check` on all crates.
- Core line coverage of at least 90%.
- Documentation: every public item with a doc comment and at least one example that compiles (doctest).

## 11. Reference scenarios

Each scenario MUST exist in `examples/<letter>_<name>.rs` with tests. At milestone M2 they are written by hand on top of `estado-core`; at M3 they are rewritten with the macro, and both versions must pass the same tests.

| ID | Scenario | What it stresses | Required behavior |
| --- | --- | --- | --- |
| A | Byte-by-byte HTTP/1 parser (`event = u8`, `effect = ()`, `infallible`) | Zero cost, events with a lifetime, `send_batch` | BM-01; `defer!` in this scenario does not compile (E0030) |
| B | Connection with retry, exponential backoff, parallel `Connected` (link + keepalive) | Epochs, regions, defer of `Send` during `Connecting` | A late timeout becomes `Stale`; `Keepalive::Dead` exits all regions in reverse order; defer overflow follows the policy |
| C | Checkout/saga: `Reserved → Authorizing → Capturing → Shipping → Done`, `Refunding` compensation | Snapshot, restore, `on_resume`, v2 → v3 migration | Restore in the middle of `Authorizing` re-emits the invoke with the same key; migration replaces `Charging` with `Authorizing` |
| D | Game character: `Alive` with regions movement, weapon, anim; `Dead` | High frequency, `cx.in_state` across regions, preemption | A fatal `Damage` exits all regions and cancels the reload; does not reload while airborne; BM-04 |
| E | Motor controller in `no_std` with `static` and `panic = "abort"` | `const fn new`, `send_with` without a buffer, `WORST_CASE_ACTIONS` | Compiles for `thumbv7em-none-eabihf`; zero allocation |
| F | UI wizard with history, `#[choice]`, `Submitting` that ignores clicks | Shallow history, choice, policy override, constructor that returns data | Going back from `Submitting` restores the step and the draft (`Draft`) without cloning |
| G | Order with N items, each a `Machine<Item>`, plus `#[child] payment` | Children, routing of timers by `ChildPath`, `on_child_done` | Order goes to `Shipped` only when all items are `Done`; the timer of a removed item becomes `Stale` |
| H | Handshake in typestate mode | `HsOut`, superstate as a trait, conversion to dynamic | Calling `send` on `ConnTs<Hs>` does not compile (`trybuild` test) |
| I | Media player (README example) | Nesting, `Fields`, constructor that moves `AudioBuffer` | Move `buf` to `Failed` without `Clone` |

## 12. Implementation plan

Seven milestones, in order. A milestone only starts when the previous one meets its definition of done. The macro comes after the core has been used by hand, so that it generates code that has already proven to be good.

1. **M0, skeleton.** Workspace, CI (clippy, fmt, tests, Miri, `thumbv7em` target), layout of `docs/`. *Done when:* CI green with empty crates.
2. **M1, risk prototypes.** Three throwaway spikes: (a) `Next<M, impl Edge>` with closures in several branches (risk R1); (b) hiding `__Poisoned` while keeping AC-15 (13.1); (c) warnings via `#[deprecated]` in a proc-macro. *Done when:* one ADR per spike with the decision.
3. **M2, hand-written core.** Complete `estado-core` (sections 4 and 5), `Driver`, basic harness. Scenarios A, B, D and I implemented by hand via `Spec`. *Done when:* AC-01 to AC-10, AC-15, AC-16, AC-18 and BM-01, BM-02.
4. **M3, macro.** `estado-macros` (section 6) generating the same code as M2. *Done when:* scenarios A, B, D and I rewritten with the macro pass the same tests; AC-17; BM-05.
5. **M4, graph and tests.** Complete `estado-graph` and `estado-test`. *Done when:* AC-14, model-based testing running on the examples, mermaid of all the examples in `docs/`.
6. **M5, runtimes and children.** `estado-tokio`, `estado-embassy`, child machines (7.4). Scenarios E and G. *Done when:* AC-05 with children, BM-04.
7. **M6, persistence, typestate and inspect.** `estado-serde`, typestate mode, `estado-inspect`. Scenarios C, F and H. *Done when:* AC-11 to AC-13, AC-19, BM-03, README with scenario I.

Each milestone ends with a single PR and a note in `CHANGELOG.md`.

## 13. Open questions and risks

[OPEN] items are decided by the implementer with an ADR. Risks have a mandatory mitigation plan.

### 13.1 Delegated decisions

| ID | Question | Options | Criterion |
| --- | --- | --- | --- |
| 13.1 | How to hide `__Poisoned` | Internal mirror enum; `Option<M>` with `take()` | Smallest `size_of` and simplest generated code; AC-15 |
| 13.2 | View of struct-like variants | Generated `PlayingMut<'a>`; require newtype | Ergonomics in example I; readable compile errors |
| 13.3 | Inline effect buffer | `smallvec` as a dependency; own implementation | Limit of two core dependencies |
| 13.4 | Region semantics | Sequential with short-circuit (current); all evaluate before applying (`#[regions(all_see_event)]`) | Prototype with scenarios B and D; keep the current one if there is no real case that breaks it |
| 13.5 | Invokes on embassy | Fixed pool; out of v0 | Complexity in M5 |
| 13.6 | Crate name | `estado` or another | Availability on crates.io |

### 13.2 Risks

| ID | Risk | Mitigation |
| --- | --- | --- |
| R1 | `Next<M, impl Edge>` with closures in several branches fails to infer in real cases (nested branches, `?`, early return) | Spike in M1. Plan B: the constructor form requires a non-capturing closure (`fn` pointer) and the edge enum stops being generic |
| R2 | Complexity and compile time of the macro | BM-05 in CI; linear generated code; `estado-core` usable without the macro |
| R3 | Parent/child split borrow (`*Fields`) causes friction when the user wants the whole `&mut Active` | Validate in scenario I; offer `#[on(Active)]` with full access when the handler belongs to the compound state itself |
| R4 | The `goto!` scan misses cases (user macros that generate `goto!`) | Document; a `goto!` generated outside the scan fails with E0040, never silently |
| R5 | Type error messages leak the internal generics | `trybuild` tests (AC-17) reviewed at each milestone; prefer errors from the macro over those from the compiler |
| R6 | Region semantics surprise those coming from XState | Documentation section comparing with SCXML/XState, with an example |
