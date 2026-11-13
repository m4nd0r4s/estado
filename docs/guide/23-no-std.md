# Using `estado` without `std`

`estado-core` is `no_std` and uses no allocator. It has no dependencies. That makes it usable on microcontrollers and in other places without an operating system. This page shows how to turn the standard library off, what changes, and what the macro and the hand-written API each give you.

## Turn `std` off

The `estado` facade and `estado-core` both have a default `std` feature. Switch the defaults off:

```toml
[dependencies]
estado = { git = "https://github.com/m4nd0r4s/estado", default-features = false }
```

Two features add pieces back:

| Feature | Adds |
| --- | --- |
| `alloc` | Effects spill to the heap (`Vec` sinks, unbounded `InlineEffects`). Needs a global allocator. |
| `std` (default) | Everything in `alloc`, plus `std::error::Error` for `SendError`. |

Without `alloc`, nothing in the core allocates. Every buffer has a size known at compile time.

## What stays bounded

| Piece | Its size |
| --- | --- |
| The machine | the root enum plus the context plus a small runtime area. |
| Effects of one `send` | `fx_inline` slots (default 4). One more effect makes `send` return `SendError::EffectOverflow`. |
| Raise and defer queues | `raise_cap` and `defer_cap` slots (default 8 each). They exist only when the machine uses them. |
| Timers, tasks, shallow history | one small counter or `Option<u8>` per state that has them. |
| Deep history | an `Option` of the substate enum of the state that has it, so as large as that enum. |

A machine that uses none of the optional features has a zero-sized runtime. Size the buffers for your worst case: `fx_inline` for the most effects one event can produce, and the two caps for the deepest queues.

## The macro works without `std`

`#[estado::machine]` generates code that uses only `core` paths. The turnstile below is ordinary macro code. Built with `default-features = false` for `thumbv7em-none-eabihf`, it compiles as it is.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Coin,
    Push,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Unlock,
    Lock,
}

// Room for the three effects this machine can emit at most per event.
#[estado::machine(event = Event, effect = Command, fx_inline = 3)]
mod turnstile {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Turnstile {
        #[initial]
        Locked,
        Unlocked,
    }

    #[handlers]
    impl Turnstile {
        #[entry(Unlocked)]
        fn enter_unlocked(cx: &mut Cx<Self>) {
            cx.emit(Command::Unlock);
        }
        #[entry(Locked)]
        fn enter_locked(cx: &mut Cx<Self>) {
            cx.emit(Command::Lock);
        }

        #[on(Locked)]
        fn on_locked(event: &Event) -> Next<Self> {
            match event {
                Event::Coin => goto!(Unlocked),
                Event::Push => unhandled!(),
            }
        }

        #[on(Unlocked)]
        fn on_unlocked(event: &Event) -> Next<Self> {
            match event {
                Event::Push => goto!(Locked),
                Event::Coin => stay!(),
            }
        }
    }
}

use turnstile::Turnstile;

fn main() {
    // `Machine::new` is a `const fn`: it can build a machine in a `static` or at start-up.
    let mut machine = Machine::<Turnstile>::new(());

    let step = machine.send(Event::Coin).unwrap();

    // Only `core` types here: `InlineEffects` iterates without allocating.
    let mut commands = step.effects.commands().copied();
    assert_eq!(commands.next(), Some(Command::Lock)); // entry of the initial state
    assert_eq!(commands.next(), Some(Command::Unlock));
    assert_eq!(commands.next(), None);
}
```

In a `no_std` program, keep these in mind:

- `std::error::Error` for `SendError` needs `std`. `Display` works everywhere.
- Timers and tasks are only effects. Your target's timer or executor carries them out, through the [`Driver`](./12-sinks-and-drivers.md) trait.
- Choose your own `Sink` when you want effects to go to a fixed array or a queue rather than the default buffer. See [Sinks and drivers](./12-sinks-and-drivers.md).

## The hand-written API

The macro writes an implementation of the `Spec` trait from `estado-core`. You can write that implementation yourself, with no macros and no proc-macro build step. The trait is the contract the macro targets, so a hand-written machine behaves exactly like a generated one and runs on the same `Machine`.

You give `Spec` its types (event, effect, context, runtime, effect buffer), a name, a static `Graph`, and three functions: how to start, how to find the active state, and how to run one **microstep**. The microstep takes the event and returns an `Outcome`.

This is a two-state machine with an entry effect, written against `estado-core` alone:

```rust
use estado_core::{
    Cx, EdgeInfo, EdgeKind, EdgeTarget, Graph, InlineEffects, Machine, NoRuntime, Outcome, Sink,
    Slot, Spec, StateId, StateInfo, StateKind, StatePath,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    Coin,
    Push,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Unlock,
    Lock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Turnstile {
    Locked,
    Unlocked,
}

impl Turnstile {
    const LOCKED: StateId = StateId::new(0);
    const UNLOCKED: StateId = StateId::new(1);
}

static STATES: [StateInfo; 2] = [
    StateInfo::new(Turnstile::LOCKED, "Locked", None, StateKind::Atomic, 0),
    StateInfo::new(Turnstile::UNLOCKED, "Unlocked", None, StateKind::Atomic, 0),
];

static GRAPH: Graph = Graph {
    machine: "Turnstile",
    states: &STATES,
    edges: &[
        EdgeInfo {
            from: Turnstile::LOCKED,
            to: EdgeTarget::State(Turnstile::UNLOCKED),
            kind: EdgeKind::Normal,
            event_hint: Some("Event::Coin"),
        },
        EdgeInfo {
            from: Turnstile::UNLOCKED,
            to: EdgeTarget::State(Turnstile::LOCKED),
            kind: EdgeKind::Normal,
            event_hint: Some("Event::Push"),
        },
    ],
};

impl Spec for Turnstile {
    type Event = Event;
    type Effect = Command;
    type Ctx = ();
    type Output = ();
    type Runtime = NoRuntime<Event>;
    type Effects = InlineEffects<Command, 4>;

    const NAME: &'static str = "Turnstile";
    const GRAPH: &'static Graph = &GRAPH;

    fn initial() -> Self {
        Self::Locked
    }

    fn state_id(&self) -> StateId {
        match self {
            Self::Locked => Self::LOCKED,
            Self::Unlocked => Self::UNLOCKED,
        }
    }

    fn path(&self) -> StatePath {
        let mut path = StatePath::new(Self::GRAPH);
        path.push(self.state_id());
        path
    }

    // The entries of the initial configuration.
    fn enter_initial<S: Sink<Command>>(&mut self, cx: &mut Cx<'_, Self, S>) {
        cx.emit(Command::Lock);
    }

    // One microstep: decide, then run the transition by hand.
    fn microstep<S: Sink<Command>>(
        slot: &mut Slot<Self>,
        cx: &mut Cx<'_, Self, S>,
        event: &Event,
    ) -> Outcome {
        match (*slot.live_mut(), event) {
            (Self::Locked, Event::Coin) => {
                slot.put(Self::Unlocked);
                cx.emit(Command::Unlock);
                Outcome::Transitioned { changed: true }
            }
            (Self::Unlocked, Event::Push) => {
                slot.put(Self::Locked);
                cx.emit(Command::Lock);
                Outcome::Transitioned { changed: true }
            }
            _ => Outcome::Stayed,
        }
    }
}

fn main() {
    let mut machine = Machine::<Turnstile>::new(());

    let step = machine.send(Event::Coin).unwrap();
    let commands: Vec<_> = step.effects.commands().copied().collect();
    assert_eq!(commands, [Command::Lock, Command::Unlock]);
    assert_eq!(machine.path().to_string(), "Unlocked");
    assert_eq!(*machine.state(), Turnstile::Unlocked);

    // The graph is plain data, readable at run time.
    assert_eq!(<Turnstile as Spec>::GRAPH.edges.len(), 2);
}
```

A real machine adds more: parent states, `#[after]` timers with epochs, history, queues. The repository's `examples/` folder holds four scenarios written both ways, once by hand against `estado-core` and once with the macro, with the same tests running on both:

- `a_http_parser` and `a_http_parser_macro`: an HTTP/1 request parser, one event per byte.
- `b_connection` and `b_connection_macro`: a connection with retry, backoff and a parallel keepalive.
- `d_game_character` and `d_game_character_macro`: a game character with three regions.
- `i_media_player` and `i_media_player_macro`: a media player with nested states.

Read a pair side by side to see what each attribute turns into.

## Choosing between the two

| | Macro | By hand |
| --- | --- | --- |
| Checking | Every `goto!` is checked at compile time. | You write the transitions yourself. |
| Graph | Built from the code, and exact. | You keep it in step with the code. |
| Build | Needs the proc-macro crate at build time. | Only `estado-core`. |
| Output | Readable, linear code. | Whatever you write. |

The macro is the normal choice. Write by hand when you cannot take a procedural-macro build step, or when you want to understand the contract.

## Next

Back to the [guide index](./README.md). The [core reference](../reference/core.md) lists the whole `estado-core` API.
