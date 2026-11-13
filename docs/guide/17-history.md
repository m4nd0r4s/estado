# History

A compound state normally forgets its substate when you leave it. Entering it again starts in its `#[initial]` substate. **History** changes that: the state remembers where it was, and a transition can ask to resume from there.

History is written when the compound state **exits**. It is read only by `goto!(State, history)`. A plain `goto!(State)` ignores it. When nothing is recorded yet, `history` falls back to the `#[initial]` substate.

There are two kinds.

| Attribute | Remembers | Costs |
| --- | --- | --- |
| `#[history]` (shallow) | which direct substate was active (one byte) | almost nothing |
| `#[history(deep, keep)]` | the whole substate value, with its data | the value is stored in the machine |

Put the attribute on the compound `#[state]` struct, next to `#[state]`.

## Shallow history

A shallow history stores only the **discriminant** of the last direct substate. The data of the compound state itself is not remembered: the `goto!` that re-enters it supplies that data again. Shallow history needs substates without data.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Open,
    Resume,
    Switch,
    Close,
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
    #[history]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Open {
        pub edits: u32,
        #[substates]
        pub mode: Mode,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Mode {
        #[initial]
        Insert,
        Command,
    }

    #[handlers]
    impl Editor {
        #[on(Closed)]
        fn on_closed(event: &Event) -> Next<Self> {
            match event {
                // A plain `goto!` starts in the initial substate.
                Event::Open => goto!(Open { edits: 0 }),
                // `history` resumes the recorded substate. The data is new.
                Event::Resume => goto!(Open { edits: 100 }, history),
                _ => unhandled!(),
            }
        }

        #[on(Insert)]
        fn on_insert(event: &Event) -> Next<Self> {
            match event {
                Event::Switch => goto!(Command),
                _ => bubble!(),
            }
        }

        #[on(Command)]
        fn on_command(event: &Event) -> Next<Self> {
            match event {
                Event::Switch => goto!(Insert),
                _ => bubble!(),
            }
        }

        #[on(Open)]
        fn on_open(_open: OpenFields<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Close => goto!(Closed),
                _ => unhandled!(),
            }
        }
    }
}

use editor::{Editor, Mode, Open};

fn main() {
    let mut machine = Machine::<Editor>::new(());

    // Nothing is recorded yet: `history` falls back to the initial substate.
    machine.send(Event::Resume).unwrap();
    assert_eq!(machine.path().to_string(), "Open/Insert");

    machine.send(Event::Switch).unwrap(); // Insert -> Command
    machine.send(Event::Close).unwrap(); // `Open` exits and records `Command`

    machine.send(Event::Resume).unwrap();
    assert_eq!(
        *machine.state(),
        Editor::Open(Open { edits: 100, mode: Mode::Command })
    );

    // A plain `goto!` ignores the record.
    machine.send(Event::Close).unwrap();
    machine.send(Event::Open).unwrap();
    assert_eq!(machine.path().to_string(), "Open/Insert");
}
```

## Deep history

`#[history(deep, keep)]` stores the **whole value** of the substate enum when the compound state exits, including the data of the substates and of any compound states below them. The value lives in the machine's runtime area, outside `state()`, not in the parent state or in the root. Re-entering with `history` takes the value out and puts it back in the state, so a recorded value is used once.

The cost is visible: the machine now holds an `Option` of that enum.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Open,
    Resume,
    Move(usize),
    Select,
    Close,
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
    #[history(deep, keep)]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Open {
        pub path: String,
        #[substates]
        pub mode: Mode,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Mode {
        #[initial(with = typing)]
        Typing { cursor: usize },
        Selecting { from: usize, to: usize },
    }

    fn typing() -> Mode {
        Mode::Typing { cursor: 0 }
    }

    #[handlers]
    impl Editor {
        #[on(Closed)]
        fn on_closed(event: &Event) -> Next<Self> {
            match event {
                Event::Open => goto!(Open { path: "a.txt".to_string() }),
                Event::Resume => goto!(Open { path: "a.txt".to_string() }, history),
                _ => unhandled!(),
            }
        }

        #[on(Typing)]
        fn on_typing(typing: TypingMut<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Move(to) => {
                    *typing.cursor = *to;
                    stay!()
                }
                Event::Select => goto!(Selecting { from: *typing.cursor, to: *typing.cursor + 5 }),
                _ => bubble!(),
            }
        }

        #[on(Selecting)]
        fn on_selecting(_selecting: SelectingMut<'_>, _event: &Event) -> Next<Self> {
            bubble!()
        }

        #[on(Open)]
        fn on_open(_open: OpenFields<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Close => goto!(Closed),
                _ => unhandled!(),
            }
        }
    }
}

use editor::{Editor, Mode};

fn main() {
    let mut machine = Machine::<Editor>::new(());
    machine.send(Event::Open).unwrap();
    machine.send(Event::Move(12)).unwrap();
    machine.send(Event::Select).unwrap();
    machine.send(Event::Close).unwrap();

    // The substate comes back with its data: from 12 to 17.
    machine.send(Event::Resume).unwrap();
    let Editor::Open(open) = machine.state() else {
        panic!("Open is active");
    };
    assert_eq!(open.mode, Mode::Selecting { from: 12, to: 17 });

    // A plain `goto!` starts over, through `#[initial(with = typing)]`.
    machine.send(Event::Close).unwrap();
    machine.send(Event::Open).unwrap();
    let Editor::Open(open) = machine.state() else {
        panic!("Open is active");
    };
    assert_eq!(open.mode, Mode::Typing { cursor: 0 });
}
```

A constructor `goto!(T, |old: Exiting![Open]| ..)` that leaves the state `Open` itself, which has deep history, still records the history. Because the substate value moves into the history, `old` is a generated struct with the remaining fields only, not the whole `Open`.

A constructor `goto!` that leaves a state which *contains* a deep history lower in the tree is [E0090](../errors/E0090.md), because the closure would receive the state by value and drop the history. Use the plain form of `goto!`, or put the history on the state named in `Exiting![..]`.

## History in nested states

History is kept per compound state. A deep history stores the value of the substate enum, and that value includes any compound states below it with their data. Resuming the outer state with `history` restores all of it.

The macro does not generate two shapes yet: a nested `#[history(deep, keep)]` inside another deep history, and `history` on a shallow history whose substates carry data. Both report [E0090](../errors/E0090.md). So does a compound state with more than 256 substates and a shallow `#[history]`, because the shallow history stores the discriminant in a `u8`.

## Next

[Defer and raise](./18-defer-and-raise.md) covers events that wait and events a machine sends to itself.
