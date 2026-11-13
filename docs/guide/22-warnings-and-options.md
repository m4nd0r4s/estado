# Warnings and options

This page covers the lints the macro emits for suspicious machines, and every option you can pass to `#[estado::machine(..)]`.

## Warnings

The macro checks the shape of your machine and warns about states that look like mistakes. Each warning has a code. They do not stop compilation.

| Code | The macro warns when |
| --- | --- |
| [W0001](../errors/W0001.md) | A state cannot be reached from the initial state by any `goto!`. |
| [W0002](../errors/W0002.md) | A state has no way out and is not `#[terminal]`: once entered, the machine can never leave it. |
| [W0003](../errors/W0003.md) | `defer!()` is used in a state that has no way out, so the event would never be redelivered. |
| [W0004](../errors/W0004.md) | The data of a variant is larger than `size_warn` bytes (default 128). Every state value takes the size of the largest variant. |
| [W0005](../errors/W0005.md) | An `#[after]` timer fires an event that no handler matches. Best effort: the macro stays quiet when it is unsure. |

A warning reads like this in your build output:

```text
warning: use of deprecated function `echo::__estado_warnings::W0002_Ready`: estado::W0002: state `Ready` has no outgoing edges and is not `#[terminal]`; once entered the machine can never leave it (silence with `#[machine(allow(W0002))]`)
```

It appears as a `deprecated` warning because that is the only way a procedural macro can emit one on stable Rust. The code is in the message. That also means `-D warnings` turns a machine warning into an error.

### `allow(Wxxxx)`

Silence a warning for one machine with `allow(..)` in the attribute. It names one or more codes and generates nothing, whatever the lint level.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Go,
}

// `Spare` is never entered (W0001) and `Sink` never leaves (W0002).
// Both are deliberate here.
#[estado::machine(event = Event, allow(W0001, W0002))]
mod odd {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Odd {
        #[initial]
        Start,
        Sink,
        Spare,
    }

    #[handlers]
    impl Odd {
        #[on(Start)]
        fn on_start(_event: &Event) -> Next<Self> {
            goto!(Sink)
        }

        #[on(Spare)]
        fn on_spare(_event: &Event) -> Next<Self> {
            goto!(Start)
        }
    }
}

use odd::Odd;

fn main() {
    let mut machine = Machine::<Odd>::new(());
    machine.send(Event::Go).unwrap();
    assert_eq!(*machine.state(), Odd::Sink);
}
```

An unknown code in `allow(..)` is an error ([E0016](../errors/E0016.md)).

Test machines often have dead ends on purpose, so `allow(W0001, W0002)` is common there. For a real machine, fix the cause first. `#![allow(deprecated)]` on the crate or module also hides every `W` code, but it hides other deprecations too.

## All options

Write options inside the attribute, separated by commas: `#[estado::machine(event = Event, effect = Command, ctx = Env)]`. An unknown or malformed option is [E0016](../errors/E0016.md), with a "did you mean" hint.

| Option | Default | Meaning |
| --- | --- | --- |
| `event = T` | required | The event type ([E0002](../errors/E0002.md) when missing). |
| `effect = T` | `()` | The type of user effects (`cx.emit`, `#[invoke]`). See [Context and effects](./11-context-and-effects.md). |
| `ctx = T` | `()` | The global context type. |
| `unhandled = ignore \| error \| panic` | `ignore` | What happens to an unhandled event. See [Unhandled events](./07-unhandled-events.md). |
| `defer_cap = N` | 8 | Capacity of the defer queue. |
| `defer_overflow = error \| drop_oldest` | `error` | What a full defer queue does. See [Defer and raise](./18-defer-and-raise.md). |
| `raise_cap = N` | 8 | Capacity of the raise queue. |
| `fx_inline = N` | 4 | Effects stored inline in a `Step` before spilling to the heap. |
| `max_microsteps = N` | 64 | Livelock limit of one `send`. See [Errors and safety](./21-errors-and-safety.md). |
| `size_warn = N` | 128 | Size in bytes above which W0004 warns. |
| `tracks_active` | off | Forces `cx.in_state` on. See below. |
| `allow(W0001, ..)` | none | Silences warnings. |

The options `typestate`, `infallible` and `version` are accepted but have no effect yet. They belong to the typestate and snapshot features of later milestones. The one check that exists today: `typestate` together with a `#[region]` field is [E0050](../errors/E0050.md).

The [configuration reference](../reference/configuration.md) lists every option with its limits.

### `fx_inline`

`step.effects` keeps up to `fx_inline` effects in the step itself. More than that spill to the heap when the `alloc` feature is on. Raise it when your machine emits many effects per event and you want to avoid allocation.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Fire,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Spark(u8),
}

#[estado::machine(event = Event, effect = Command, fx_inline = 2)]
mod burst {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Burst {
        #[initial]
        Ready,
        Spent,
    }

    #[handlers]
    impl Burst {
        #[on(Ready)]
        fn on_ready(cx: &mut Cx<Self>, _event: &Event) -> Next<Self> {
            for number in 0..5 {
                cx.emit(Command::Spark(number));
            }
            goto!(Spent)
        }

        #[on(Spent)]
        fn on_spent(_event: &Event) -> Next<Self> {
            goto!(Ready)
        }
    }
}

use burst::Burst;

fn main() {
    let mut machine = Machine::<Burst>::new(());
    let step = machine.send(Event::Fire).unwrap();
    // Two effects fit inline; the rest went to the heap. All five are there.
    assert_eq!(step.effects.len(), 5);
}
```

Without the `alloc` feature there is no heap. Pick `fx_inline` large enough, or `send` returns `SendError::EffectOverflow`. See [no_std](./23-no-std.md).

### `size_warn`

A machine is one enum, so every state value is as large as the largest variant. W0004 reminds you to box a big payload. Raise `size_warn` when the size is intended.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Load,
}

// 512 bytes of samples are fine here: raise the limit instead of boxing.
#[estado::machine(event = Event, size_warn = 1024)]
mod recorder {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Recorder {
        #[initial]
        Empty,
        Loaded { samples: [u8; 512] },
    }

    #[handlers]
    impl Recorder {
        #[on(Empty)]
        fn on_empty(_event: &Event) -> Next<Self> {
            goto!(Loaded { samples: [0; 512] })
        }

        #[on(Loaded)]
        fn on_loaded(_loaded: LoadedMut<'_>, _event: &Event) -> Next<Self> {
            goto!(Empty)
        }
    }
}

use recorder::Recorder;

fn main() {
    let mut machine = Machine::<Recorder>::new(());
    machine.send(Event::Load).unwrap();
    assert!(matches!(machine.state(), Recorder::Loaded { .. }));
}
```

## Feature detection

The macro builds the raise and defer queues, and the active-configuration snapshot behind `cx.in_state`, only when it sees them used. It looks inside the machine module for `cx.raise(..)`, `defer!()` and `cx.in_state(..)`, including the path forms `Cx::raise(cx, ..)` and `Cx::in_state(cx, ..)`. A helper function **inside** the module is found. A helper **outside** the module is not.

For a helper outside the module, switch the feature on by hand:

- `raise_cap = N` or `defer_cap = N` always generates the queues, even when you write the default value.
- `tracks_active` turns `cx.in_state` on.

Without that, the first `raise` in the helper overflows an empty queue and poisons the machine, and `in_state` answers `false` for ever.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Toggle,
    Flash,
    Probe,
}

/// Outside the machine module, written as a path call: the macro's scan of the
/// module sees neither of these.
fn flash<S: Sink<()>>(cx: &mut Cx<'_, lamp::Lamp, S>) {
    Cx::raise(cx, Event::Flash);
}

fn is_on<S: Sink<()>>(cx: &Cx<'_, lamp::Lamp, S>) -> bool {
    Cx::in_state(cx, lamp::Lamp::ON)
}

#[estado::machine(event = Event, ctx = Vec<String>, raise_cap = 2, tracks_active)]
mod lamp {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Lamp {
        #[initial]
        Off,
        On,
    }

    #[handlers]
    impl Lamp {
        #[on(Off)]
        fn on_off(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Toggle => {
                    flash(cx);
                    goto!(On)
                }
                _ => unhandled!(),
            }
        }

        #[on(On)]
        fn on_on(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Flash => {
                    cx.ctx_mut().push("flash".to_string());
                    stay!()
                }
                Event::Probe => {
                    let line = if is_on(cx) { "probe: on" } else { "probe: off" };
                    cx.ctx_mut().push(line.to_string());
                    stay!()
                }
                Event::Toggle => goto!(Off),
            }
        }
    }
}

use lamp::Lamp;

fn main() {
    let mut machine = Machine::<Lamp>::new(Vec::new());

    // The raise from the helper has its queue and is delivered in the same `send`.
    machine.send(Event::Toggle).unwrap();
    assert!(!machine.is_poisoned());
    assert_eq!(machine.ctx(), &["flash"]);

    // `in_state` from a helper sees the active configuration.
    machine.send(Event::Probe).unwrap();
    assert_eq!(machine.ctx(), &["flash", "probe: on"]);
}
```

Note the helper's signature. Outside the module you spell the context type in full, `Cx<'_, Lamp, S>`, where `S` is a sink for the machine's effect type (`Sink<()>` here, because the machine has no effects).

## Next

[no_std](./23-no-std.md) shows how to use the core without the standard library.
