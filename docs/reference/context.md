# Context

`Cx` is the value a handler receives as `cx: &mut Cx<Self>`. It gives access to the global context, the effect sink, the internal event queue and the active configuration for the length of one microstep. Entries, exits, `#[always]` and `#[on_done]` handlers receive it too.

```rust,ignore
pub struct Cx<'step, M: Spec, S> { /* private */ }
```

In a handler you write `Cx<Self>`. The macro adds the lifetime and the sink parameter. In a helper function outside the handlers, write all three:

```rust,ignore
fn note<S: Sink<Command>>(cx: &mut Cx<'_, Player, S>, line: &str) { /* .. */ }
```

`Cx` is created by `Machine`. You cannot build one, except in tests of a hand-written [`Spec`](core.md).

## Methods

| Method | Signature | Purpose |
| --- | --- | --- |
| `ctx` | `fn ctx(&self) -> &M::Ctx` | Read the global context. |
| `ctx_mut` | `fn ctx_mut(&mut self) -> &mut M::Ctx` | Change the global context. |
| `emit` | `fn emit(&mut self, command: M::Effect)` | Emit a user effect. |
| `raise` | `fn raise(&mut self, event: M::Event)` | Queue an internal event. |
| `in_state` | `fn in_state(&self, state: StateId) -> bool` | Whether a state is active. |

### ctx and ctx_mut

The global context is the value you passed to `Machine::new(ctx)`, of the type given by the `ctx` option (`()` by default). It is separate from the data inside states, and it lives as long as the machine. `Machine::ctx()` reads it from outside.

### emit

`emit(command)` pushes `Effect::User(command)` to the sink, after the effects that came before it. `Step::effects` and the sink you passed to `send_into` receive it. The type is the `effect` option.

If the sink refuses it ([`Overflow`](effects.md#sink)), the step reports `SendError::EffectOverflow` when the microstep ends, and the machine is poisoned. See [errors](errors.md#senderror).

### raise

`raise(event)` queues an internal event. The machine processes it:

- after the current microstep has finished, including its exits and entries;
- after pending `#[always]` evaluations;
- before the deferred events are redelivered;
- in the order raised, within the same `send`.

An internal event that nobody handles is not an error; it is counted in `report.unhandled_internal`. A raised event counts toward the [livelock budget](errors.md#the-livelock-budget).

The queue holds `raise_cap` events (default 8). A full queue makes the step fail with `SendError::RaiseOverflow` and poisons the machine.

`raise` is the way to make another region react: regions never transition each other directly ([`E0021`](../errors/E0021.md)).

The macro enables the queue when it sees `cx.raise(..)` in the module. For a call from a helper outside the module, set `raise_cap = N` in `#[machine(..)]`.

### in_state

`in_state(Root::STATE)` tells a handler whether a state is active, including states in the sibling regions of a parallel state. It reads the configuration at the start of the microstep, and a later region sees the transitions that earlier regions of the same event already made.

- It needs the snapshot that `#[machine]` keeps only when it sees `cx.in_state(..)` in the module, or when you write `tracks_active`. Without the snapshot it returns `false` for every state.
- It is available in `#[always]` handlers as well.

## State ids

`#[machine]` adds one associated constant of type `StateId` per state and per region to the root type. The name is the state name in SCREAMING_SNAKE_CASE:

| State | Constant |
| --- | --- |
| `Off` | `Player::OFF` |
| `PlayingFast` | `Player::PLAYING_FAST` |
| region field `keep_alive` | `Player::KEEP_ALIVE` |
| `HTTPServer` | `Player::HTTP_SERVER` |

`StateId` is `Copy`, `Eq`, `Ord` and `Hash`. `StateId::new(index)` builds one from a graph index and `index()` returns it. Ids are used with `Cx::in_state`, `Machine::is_in` and `StatePath::contains`. A constant that would equal a variant name of the root enum is [`E0012`](../errors/E0012.md).

To get the name or the parent of an id, use the static graph: `Root::GRAPH.state(Root::OFF)` returns a `StateInfo` with `name`, `parent`, `kind`, `initial`, `history`, `timers`, `invokes` and `depth`. See [core](core.md#graph).

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Switch,
    Ping,
    Pong,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Log(&'static str),
}

#[estado::machine(event = Event, effect = Command, ctx = u32, allow(W0001, W0002))]
mod panel {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Panel {
        #[initial]
        Running(Running),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Running {
        #[region]
        pub light: Light,
        #[region]
        pub mode: Mode,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Light {
        #[initial]
        Off,
        On,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Mode {
        #[initial]
        Idle,
        Lit,
        Dark,
    }

    #[handlers]
    impl Panel {
        #[on(Off)]
        fn on_off(event: &Event) -> Next<Self> {
            match event {
                Event::Switch => goto!(On),
                _ => bubble!(),
            }
        }

        #[on(Idle)]
        fn on_idle(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Ping => {
                    *cx.ctx_mut() += 1; // the global context
                    cx.emit(Command::Log("ping")); // an effect
                    cx.raise(Event::Pong); // an internal event
                    stay!()
                }
                // The raised event, processed in the same `send`.
                Event::Pong => {
                    if cx.in_state(Panel::ON) {
                        // a state of the sibling region
                        goto!(Lit)
                    } else {
                        goto!(Dark)
                    }
                }
                _ => bubble!(),
            }
        }
    }
}

use panel::Panel;

fn main() {
    let mut machine = Machine::<Panel>::new(0);
    machine.send(Event::Switch).unwrap();

    let step = machine.send(Event::Ping).unwrap();
    assert_eq!(step.effects.commands().collect::<Vec<_>>(), [&Command::Log("ping")]);
    assert_eq!(step.report.microsteps, 2); // `Ping`, then the raised `Pong`
    assert_eq!(*machine.ctx(), 1);
    assert_eq!(machine.path().to_string(), "Running/light:On,mode:Lit");
    assert!(machine.is_in(Panel::LIT));
}
```
