# States

A machine is a tree of states. This page shows how to declare that tree, how states carry data and which views the macro generates so handlers can reach that data.

## The root enum

Every machine has one `#[root]` enum. Its variants are the top-level states.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Start,
    Stop,
}

#[estado::machine(event = Event)]
mod job {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Job {
        #[initial]
        Waiting,
        Running { attempts: u8 },
        Archived(String),
    }

    #[handlers]
    impl Job {
        #[on(Waiting)]
        fn on_waiting(event: &Event) -> Next<Self> {
            match event {
                Event::Start => goto!(Running { attempts: 1 }),
                _ => unhandled!(),
            }
        }

        #[on(Running)]
        fn on_running(running: RunningMut<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Start => {
                    *running.attempts += 1;
                    stay!()
                }
                Event::Stop => goto!(Archived("stopped".to_string())),
                _ => unhandled!(),
            }
        }

        #[on(Archived)]
        fn on_archived(_note: &mut String, _event: &Event) -> Next<Self> {
            goto!(Waiting)
        }
    }
}

use job::Job;

fn main() {
    let mut machine = Machine::<Job>::new(());
    machine.send(Event::Start).unwrap();
    machine.send(Event::Start).unwrap();
    assert_eq!(*machine.state(), Job::Running { attempts: 2 });
    machine.send(Event::Stop).unwrap();
    assert_eq!(*machine.state(), Job::Archived("stopped".to_string()));
}
```

A state can be a unit variant (`Waiting`), a struct-like variant (`Running { attempts }`) or a tuple variant (`Archived(String)`). Items in the module that are not part of the machine, such as helper functions and constants, pass through unchanged.

Rules that apply to every level:

- State names are unique across the whole machine, at every level. A repeated name is [E0012](../errors/E0012.md), because `goto!(Name)` must be unambiguous.
- The tree is at most 16 levels deep ([E0011](../errors/E0011.md)).
- A level is a state enum. Every state enum has exactly one `#[initial]` variant ([E0010](../errors/E0010.md)).

## `#[initial]`

`#[initial]` marks the variant a level starts in. For the root, that is the state the machine is in after the start. For a substate enum, it is the substate entered when the parent is entered without naming a substate.

A state with fields cannot start from nothing. If the `#[initial]` variant has data, you need a way to build it:

- `#[initial(with = path)]` names a function that returns the value of the enum the variant belongs to. The macro calls it when the state is entered through its initial. The function takes no arguments, or the `Cx` and the fields of the parent state (see [Reading the context and the parent fields](#reading-the-context-and-the-parent-fields)).
- Or every `goto!` that enters the state gives the data explicitly, with the path form described in [Transitions](./05-transitions.md). Entering such a state through its initial without either is [E0020](../errors/E0020.md).

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Open,
}

#[estado::machine(event = Event)]
mod editor {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Editor {
        #[initial]
        Closed,
        Open(Open),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Open {
        pub path: String,
        #[substates]
        pub mode: Mode,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Mode {
        // `Insert` has data, so the initial needs a constructor.
        #[initial(with = default_insert)]
        Insert { cursor: usize },
        Command,
    }

    fn default_insert() -> Mode {
        Mode::Insert { cursor: 0 }
    }

    #[handlers]
    impl Editor {
        #[on(Closed)]
        fn on_closed(event: &Event) -> Next<Self> {
            match event {
                // No substate named: `Mode` starts in its `#[initial]`.
                Event::Open => goto!(Open { path: "notes.txt".to_string() }),
            }
        }

        #[on(Insert)]
        fn on_insert(_event: &Event) -> Next<Self> {
            goto!(Command)
        }

        #[on(Command)]
        fn on_command(_event: &Event) -> Next<Self> {
            goto!(Insert { cursor: 0 })
        }
    }
}

use editor::{Editor, Mode, Open};

fn main() {
    let mut machine = Machine::<Editor>::new(());
    machine.send(Event::Open).unwrap();
    assert_eq!(
        *machine.state(),
        Editor::Open(Open {
            path: "notes.txt".to_string(),
            mode: Mode::Insert { cursor: 0 },
        })
    );
}
```

The constructor above takes no arguments. When the initial value depends on the context, on effects, or on the fields of the parent, give the function parameters instead (below).

`#[initial(with = ..)]` works on the root enum too. The function returns a value of the root enum, and the machine calls it when it starts:

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Loaded,
}

#[estado::machine(event = Event)]
mod boot {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Boot {
        // The machine starts in `Loading` with three retries left.
        #[initial(with = loading)]
        Loading { retries: u8 },
        Ready,
    }

    fn loading() -> Boot {
        Boot::Loading { retries: 3 }
    }

    #[handlers]
    impl Boot {
        #[on(Loading)]
        fn on_loading(_loading: LoadingMut<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Loaded => goto!(Ready),
            }
        }

        #[on(Ready)]
        fn on_ready(_event: &Event) -> Next<Self> {
            goto!(Loading { retries: 3 })
        }
    }
}

use boot::Boot;

fn main() {
    let mut machine = Machine::<Boot>::new(());
    machine.start().unwrap();
    assert_eq!(*machine.state(), Boot::Loading { retries: 3 });
}
```

### Reading the context and the parent fields

A function that takes parameters is called when the state is entered, after the states that the transition leaves have exited and before the new states run their entries. It can read and write the global context, emit effects and raise events through `cx`, and it receives the fields of the parent state. Its parameters are recognised by their types, in any order:

| Parameter | What it is |
| --- | --- |
| `&mut Cx<'_, Root, S>` | The context, as in a handler. The function is generic over the sink: `fn f<S: Sink<Effect>>(cx: &mut Cx<'_, Root, S>)`. |
| `<Parent>Fields<'_>` | `&mut` references to the shared fields of the compound or parallel state that the initial belongs to, for example `ActiveFields<'_>` for a `#[substates]` field of `Active`. Not available on the root enum, which has no parent. |

The function returns the value of the enum, like the zero-argument form. The macro reads the signature from the machine module: a free function (`with = f`) or a function of the root's `#[handlers]` impl (`with = Self::f`). A path to anything else is called without arguments.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Open,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Log(String),
}

#[estado::machine(event = Event, effect = Command, ctx = u32, allow(W0002))]
mod editor {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Editor {
        #[initial]
        Closed,
        Open(Open),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Open {
        pub path: String,
        #[substates]
        pub mode: Mode,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Mode {
        #[initial(with = resume)]
        Insert { cursor: usize },
        Command,
    }

    // The cursor is where the last session stopped: it lives in the context.
    // The function also logs which file it opened, an effect.
    fn resume<S: Sink<Command>>(cx: &mut Cx<'_, Editor, S>, open: OpenFields<'_>) -> Mode {
        cx.emit(Command::Log(format!("resume {}", open.path)));
        Mode::Insert {
            cursor: *cx.ctx() as usize,
        }
    }

    #[handlers]
    impl Editor {
        #[on(Closed)]
        fn on_closed(event: &Event) -> Next<Self> {
            match event {
                Event::Open => goto!(Open { path: "notes.txt".to_string() }),
            }
        }

        #[on(Open)]
        fn on_open(_event: &Event) -> Next<Self> {
            stay!()
        }
    }
}

use editor::{Editor, Mode, Open};

fn main() {
    let mut machine = Machine::<Editor>::new(42);
    let step = machine.send(Event::Open).unwrap();

    // `resume` ran when `Open` was entered: the effect is in the step.
    let logged = step
        .effects
        .into_iter()
        .any(|effect| effect == Effect::User(Command::Log("resume notes.txt".to_string())));
    assert!(logged);
    assert_eq!(
        *machine.state(),
        Editor::Open(Open {
            path: "notes.txt".to_string(),
            mode: Mode::Insert { cursor: 42 },
        })
    );
}
```

The same function runs wherever the state is entered through its initial: a `goto!` that does not name the substate, `reenter!()`, and a local `goto!` to an ancestor, which gives the function the live fields of that ancestor. On the root enum the function takes at most the `Cx` and runs inside `Machine::start`, before the first entries. A panic in the function, or an effect the sink refuses, poisons the machine like any other user code of a start.

One combination is not supported yet: `goto!(State, history)` into a state whose initial takes parameters is [E0090](../errors/E0090.md), because the function would run even when a history is recorded and replaces its result.

## Compound states and `#[substates]`

A **compound state** is a state with substates. You declare it as a `#[state]` struct. It holds the data shared by all its substates plus exactly one `#[substates]` field. The field type is another `#[state]` enum with its own `#[initial]`.

In the parent enum, the variant wraps the struct: `Active(Active)`. Substates nest the same way, as deep as you need.

```rust
use core::time::Duration;
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Play(u32),
    Pause,
    Resume,
}

#[estado::machine(event = Event)]
mod player {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Player {
        #[initial]
        Idle,
        Active(Active),
    }

    /// Shared data of every substate lives in the struct.
    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Active {
        pub track: u32,
        #[substates]
        pub sub: ActiveSub,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum ActiveSub {
        #[initial]
        Playing { position: Duration },
        Paused { position: Duration },
    }

    #[handlers]
    impl Player {
        #[on(Idle)]
        fn on_idle(event: &Event) -> Next<Self> {
            match event {
                Event::Play(track) => {
                    goto!(Active { track: *track } / Playing { position: Duration::ZERO })
                }
                _ => unhandled!(),
            }
        }

        #[on(Playing)]
        fn on_playing(playing: PlayingMut<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Pause => goto!(Paused { position: *playing.position }),
                _ => bubble!(),
            }
        }

        #[on(Paused)]
        fn on_paused(paused: PausedMut<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Resume => goto!(Playing { position: *paused.position }),
                _ => bubble!(),
            }
        }
    }
}

use player::Player;

fn main() {
    let mut machine = Machine::<Player>::new(());
    machine.send(Event::Play(3)).unwrap();
    assert_eq!(machine.path().to_string(), "Active/Playing");

    machine.send(Event::Pause).unwrap();
    assert_eq!(machine.path().to_string(), "Active/Paused");
    assert!(machine.is_in(Player::ACTIVE_ID));
    assert!(machine.is_in(Player::PAUSED_ID));
    assert!(!machine.is_in(Player::PLAYING_ID));
}
```

`path()` renders the active states from the root down, separated by `/`. `machine.is_in(id)` tests one state. Each state has an id constant on the root type, in SCREAMING_SNAKE_CASE with the suffix `_ID`: `Player::ACTIVE_ID`, `Player::PLAYING_ID`. Ids follow the tree in pre-order, starting at 0. The suffix keeps the constant apart from the variants, so state names such as `A` or `IO` work.

A struct cannot mix `#[substates]` with `#[region]` fields ([E0013](../errors/E0013.md)). Regions make a parallel state, covered in [Regions](./13-regions.md).

## `#[terminal]` states

`#[terminal]` marks a final state. A terminal state in the root ends the machine: `is_done()` becomes `true`, and later `send` calls return `Ok` with `ignored = Unhandled`, whatever the policy. A terminal state must not transition out ([E0022](../errors/E0022.md)).

A terminal variant can carry data. For the root, a **newtype** payload (`Done(Receipt)`) is the machine's **output**, which you take with `take_output()`. A struct-like or tuple terminal variant has no output: when no root terminal has a newtype payload, `take_output()` returns `Some(())`.

The same rule applies to `#[on_done]`: only the newtype payload of a terminal state is passed to the handler.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Finish(u32),
}

#[derive(Debug, PartialEq, Eq)]
pub struct Receipt {
    pub units: u32,
}

#[estado::machine(event = Event)]
mod order {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Order {
        #[initial]
        Open,
        #[terminal]
        Closed(Receipt),
    }

    #[handlers]
    impl Order {
        #[on(Open)]
        fn on_open(event: &Event) -> Next<Self> {
            match event {
                Event::Finish(units) => goto!(Closed(Receipt { units: *units })),
            }
        }
    }
}

use order::Order;

fn main() {
    let mut machine = Machine::<Order>::new(());
    assert_eq!(machine.take_output(), None); // not done yet

    machine.send(Event::Finish(4)).unwrap();
    assert!(machine.is_done());
    assert_eq!(machine.take_output(), Some(Receipt { units: 4 }));

    // The output left the machine. It stays done and ignores events.
    assert_eq!(machine.take_output(), None);
    assert!(machine.is_done());
}
```

A terminal inside a compound state completes that state instead. See [On done](./10-on-done.md).

## Next

[State data and views](./03-state-data.md) shows how states carry data and how handlers reach it.
