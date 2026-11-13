# Context and effects

A machine never talks to the outside world itself. It reads a **context** you give it and it returns **effects** that you run. This keeps the machine deterministic and easy to test. Time, I/O and async work all live outside.

## The global context

The context is data that every handler can read and write, such as configuration or counters. Declare its type with `ctx = ...` and pass a value to `Machine::new`:

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Connect,
    Retry,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Env {
    pub max_attempts: u8,
    pub attempts: u8,
}

#[estado::machine(event = Event, ctx = Env)]
mod client {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Client {
        #[initial]
        Idle,
        Connecting,
        GaveUp,
    }

    #[handlers]
    impl Client {
        #[on(Idle)]
        fn on_idle(cx: &mut Cx<Self>, _event: &Event) -> Next<Self> {
            cx.ctx_mut().attempts = 1;
            goto!(Connecting)
        }

        #[on(Connecting)]
        fn on_connecting(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Retry if cx.ctx().attempts >= cx.ctx().max_attempts => goto!(GaveUp),
                Event::Retry => {
                    cx.ctx_mut().attempts += 1;
                    stay!()
                }
                _ => bubble!(),
            }
        }

        #[on(GaveUp)]
        fn on_gave_up(_event: &Event) -> Next<Self> {
            goto!(Idle)
        }
    }
}

use client::Client;

fn main() {
    let env = Env { max_attempts: 2, attempts: 0 };
    let mut machine = Machine::<Client>::new(env);

    machine.send(Event::Connect).unwrap(); // Idle -> Connecting, attempt 1
    machine.send(Event::Retry).unwrap(); // attempt 2
    machine.send(Event::Retry).unwrap(); // out of attempts
    assert_eq!(*machine.state(), Client::GaveUp);
    assert_eq!(machine.ctx().attempts, 2);
}
```

Without `ctx = ...` the context is `()`, and you pass `()` to `Machine::new`. The machine also exposes `ctx()` and `ctx_mut()`, so code outside the machine can read and change the context between events.

## `Cx`: what a handler can do

A handler, an entry, an exit, an `#[always]` and an `#[on_done]` can ask for `cx: &mut Cx<Self>`. It offers four things:

| Call | What it does |
| --- | --- |
| `cx.ctx()` / `cx.ctx_mut()` | Read or write the global context. |
| `cx.emit(command)` | Records a user effect. It leaves the machine in the step's `effects`. |
| `cx.raise(event)` | Queues an internal event for the same run. See [Defer and raise](./18-defer-and-raise.md). |
| `cx.in_state(Machine::STATE)` | `true` when that state is active, including states in sibling regions. See [Regions](./13-regions.md). |

Inside the machine module, write `&mut Cx<Self>` and the macro fills in the rest. In a helper function outside the module, you spell the full type, `Cx<'_, M, S>`, where `S` is any sink for the machine's effect type:

```text
fn note<S: Sink<Command>>(cx: &mut Cx<'_, client::Client, S>) { cx.emit(Command::Log); }
```

Here is a helper that emits effects. It is generic over the sink, so it works with whatever sink `send` uses:

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Press,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Beep(u8),
    Log(String),
}

// Outside the module: spell the whole `Cx` type.
fn announce<S: Sink<Command>>(cx: &mut Cx<'_, buzzer::Buzzer, S>, presses: u8) {
    cx.emit(Command::Beep(presses));
    cx.emit(Command::Log(format!("press {presses}")));
}

#[estado::machine(event = Event, effect = Command, ctx = u8)]
mod buzzer {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Buzzer {
        #[initial]
        Quiet,
        Loud,
    }

    #[handlers]
    impl Buzzer {
        #[on(Quiet)]
        fn on_quiet(cx: &mut Cx<Self>, _event: &Event) -> Next<Self> {
            *cx.ctx_mut() += 1;
            let presses = *cx.ctx();
            announce(cx, presses);
            goto!(Loud)
        }

        #[on(Loud)]
        fn on_loud(cx: &mut Cx<Self>, _event: &Event) -> Next<Self> {
            *cx.ctx_mut() += 1;
            let presses = *cx.ctx();
            announce(cx, presses);
            goto!(Quiet)
        }
    }
}

use buzzer::Buzzer;

fn main() {
    let mut machine = Machine::<Buzzer>::new(0);
    machine.send(Event::Press).unwrap();
    let step = machine.send(Event::Press).unwrap();
    assert_eq!(
        step.effects.commands().collect::<Vec<_>>(),
        [&Command::Beep(2), &Command::Log("press 2".to_string())]
    );
}
```

A helper outside the module that calls `cx.raise` or `cx.in_state` is invisible to the macro, which looks for those calls in the module. Turn the features on by hand. See [Warnings and options](./22-warnings-and-options.md#feature-detection).

## Effects from entries, exits and automatic handlers

`cx.emit` works anywhere `cx` is available: handlers, entries, exits, `#[always]` and `#[on_done]`. The effects come out in the order the code ran. For one transition that is: the effects of the exits (deepest first), then the effects of the entries (outermost first):

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Switch,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Say(&'static str),
}

#[estado::machine(event = Event, effect = Command)]
mod stage {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Stage {
        #[initial]
        Left,
        Right,
    }

    #[handlers]
    impl Stage {
        #[exit(Left)]
        fn exit_left(cx: &mut Cx<Self>) {
            cx.emit(Command::Say("exit Left"));
        }
        #[entry(Right)]
        fn enter_right(cx: &mut Cx<Self>) {
            cx.emit(Command::Say("enter Right"));
        }

        #[on(Left)]
        fn on_left(cx: &mut Cx<Self>, _event: &Event) -> Next<Self> {
            cx.emit(Command::Say("handler Left"));
            goto!(Right)
        }

        #[on(Right)]
        fn on_right(_event: &Event) -> Next<Self> {
            goto!(Left)
        }
    }
}

use stage::Stage;

fn main() {
    let mut machine = Machine::<Stage>::new(());
    let step = machine.send(Event::Switch).unwrap();
    assert_eq!(
        step.effects.commands().collect::<Vec<_>>(),
        [&Command::Say("handler Left"), &Command::Say("exit Left"), &Command::Say("enter Right")]
    );
}
```

## Effects

An **effect** is something the machine asks its runtime to do. There are five kinds, all in the `Effect<Command>` enum:

| Effect | Produced by |
| --- | --- |
| `Effect::User(command)` | `cx.emit(command)` |
| `Effect::StartTimer { key, after }` | entering a state with `#[after]` |
| `Effect::CancelTimer(key)` | leaving such a state |
| `Effect::Invoke { key, task }` | entering a state with `#[invoke]` |
| `Effect::Cancel(key)` | leaving such a state |

`Command` is the type you name with `effect = ...`. Without it, it is `()`. [Timers](./15-timers.md) and [Tasks](./16-tasks.md) explain the last four.

`send` collects the effects of a whole run in `step.effects`, in the order the machine produced them. That value is an `InlineEffects`. It stores up to `fx_inline` effects (default 4) without allocating, then spills to the heap when the `alloc` feature is on. Without `alloc`, an effect that does not fit makes `send` return `SendError::EffectOverflow` and poisons the machine.

```rust
use estado::prelude::*;
use estado::Effect;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Send(String),
    Close,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Write(String),
    Flush,
}

#[estado::machine(event = Event, effect = Command)]
mod writer {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Writer {
        #[initial]
        Ready,
        #[terminal]
        Closed,
    }

    #[handlers]
    impl Writer {
        #[on(Ready)]
        fn on_ready(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Send(text) => {
                    cx.emit(Command::Write(text.clone()));
                    cx.emit(Command::Flush);
                    stay!()
                }
                Event::Close => goto!(Closed),
            }
        }
    }
}

use writer::Writer;

fn main() {
    let mut machine = Machine::<Writer>::new(());
    let step = machine.send(Event::Send("hello".to_string())).unwrap();

    assert_eq!(step.effects.len(), 2);
    // `commands()` keeps only the user effects.
    assert_eq!(
        step.effects.commands().collect::<Vec<_>>(),
        [&Command::Write("hello".to_string()), &Command::Flush]
    );
    // `iter()` shows every effect, timers and tasks included.
    assert!(step.effects.iter().all(|effect| matches!(effect, Effect::User(_))));
}
```

## Next

[Sinks and drivers](./12-sinks-and-drivers.md) shows where effects can go.
