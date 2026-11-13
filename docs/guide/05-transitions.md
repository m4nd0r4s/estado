# Transitions

This page covers `goto!` in all its forms, `reenter!()` and the order in which states exit and enter.

## `goto!`

`goto!` names the target state. The macro checks at compile time that the target exists ([E0003](../errors/E0003.md)) and that you supplied all the data it needs ([E0020](../errors/E0020.md)). The whole transition is resolved before any exit or entry runs, so the graph the macro builds is exact.

### Simple form

Name the state and give the data it needs:

- `goto!(Unit)` for a unit state;
- `goto!(Running { attempts: 1 })` for a struct-like state;
- `goto!(Archived(note))` for a tuple state.

When the target is a compound state, its substates start in their `#[initial]`. If the target is an ancestor of the handler's state, you give no data, because that ancestor is not rebuilt.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Unit,
    Struct(u8),
    Tuple(String),
    Compound,
    Up,
}

#[estado::machine(event = Event)]
mod forms {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Forms {
        #[initial]
        Hub,
        Plain,
        Running { attempts: u8 },
        Archived(String),
        Group(Group),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Group {
        pub label: String,
        #[substates]
        pub member: Member,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Member {
        #[initial]
        First,
        Second,
    }

    #[handlers]
    impl Forms {
        #[on(Hub)]
        fn on_hub(event: &Event) -> Next<Self> {
            match event {
                Event::Unit => goto!(Plain),
                Event::Struct(attempts) => goto!(Running { attempts: *attempts }),
                Event::Tuple(note) => goto!(Archived(note.clone())),
                // The substates start in their initial: `First`.
                Event::Compound => goto!(Group { label: "g".to_string() }),
                Event::Up => unhandled!(),
            }
        }

        #[on(Plain)]
        fn on_plain(_event: &Event) -> Next<Self> {
            goto!(Hub)
        }

        #[on(Running)]
        fn on_running(_running: RunningMut<'_>, _event: &Event) -> Next<Self> {
            goto!(Hub)
        }

        #[on(Archived)]
        fn on_archived(_note: &mut String, _event: &Event) -> Next<Self> {
            goto!(Hub)
        }

        #[on(First)]
        fn on_first(event: &Event) -> Next<Self> {
            match event {
                Event::Up => goto!(Second),
                _ => bubble!(),
            }
        }

        // `Group` is an ancestor of `Second`: no data needed, it is not rebuilt.
        #[on(Second)]
        fn on_second(_event: &Event) -> Next<Self> {
            goto!(Group)
        }

        #[on(Group)]
        fn on_group(_group: GroupFields<'_>, _event: &Event) -> Next<Self> {
            goto!(Hub)
        }
    }
}

use forms::{Forms, Group, Member};

fn main() {
    let mut machine = Machine::<Forms>::new(());

    machine.send(Event::Struct(3)).unwrap();
    assert_eq!(*machine.state(), Forms::Running { attempts: 3 });
    machine.send(Event::Unit).unwrap(); // any event leaves `Running`

    machine.send(Event::Tuple("note".to_string())).unwrap();
    assert_eq!(*machine.state(), Forms::Archived("note".to_string()));
    machine.send(Event::Unit).unwrap();

    machine.send(Event::Compound).unwrap();
    assert_eq!(machine.path().to_string(), "Group/First");
    machine.send(Event::Up).unwrap();
    assert_eq!(machine.path().to_string(), "Group/Second");

    // Going to the ancestor restarts the substates but keeps the `Group` data.
    machine.send(Event::Unit).unwrap();
    assert_eq!(
        *machine.state(),
        Forms::Group(Group { label: "g".to_string(), member: Member::First })
    );
}
```

### Path form

To enter a compound state at a deep substate, write the path with `/`. Each level gets the data it needs:

```text
goto!(Active { track: 7 } / Playing { position: Duration::ZERO })
```

This enters `Active`, then `Playing`. For a parallel state, the path skips the regions and names a state inside one of them. The other regions start in their `#[initial]`. See [Regions](./13-regions.md).

A path can be as deep as the tree. Each level that has data gets its own braces. Levels you skip, below the last one you name, start in their `#[initial]`:

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Public,
    Shallow,
    Deep,
}

#[estado::machine(event = Event, ctx = Vec<String>)]
mod site {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Site {
        #[initial]
        Closed,
        Open(Open),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Open {
        pub host: String,
        #[substates]
        pub area: Area,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Area {
        #[initial]
        Public,
        Admin(Admin),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Admin {
        pub user: String,
        #[substates]
        pub page: Page,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Page {
        #[initial]
        Dashboard,
        Settings { tab: u8 },
    }

    #[handlers]
    impl Site {
        #[entry(Open)]
        fn enter_open(open: OpenFields<'_>, cx: &mut Cx<Self>) {
            cx.ctx_mut().push(format!("enter Open {}", open.host));
        }
        #[entry(Admin)]
        fn enter_admin(admin: AdminFields<'_>, cx: &mut Cx<Self>) {
            cx.ctx_mut().push(format!("enter Admin {}", admin.user));
        }
        #[entry(Dashboard)]
        fn enter_dashboard(cx: &mut Cx<Self>) {
            cx.ctx_mut().push("enter Dashboard".to_string());
        }
        #[entry(Settings)]
        fn enter_settings(settings: SettingsMut<'_>, cx: &mut Cx<Self>) {
            cx.ctx_mut().push(format!("enter Settings tab {}", settings.tab));
        }

        #[on(Closed)]
        fn on_closed(event: &Event) -> Next<Self> {
            match event {
                Event::Public => goto!(Open { host: "c.org".to_string() }),
                // Data for `Open` and `Admin`; `Page` starts in `Dashboard`.
                Event::Shallow => goto!(Open { host: "a.org".to_string() } / Admin { user: "ada".to_string() }),
                // Data at every level, down to `Settings`.
                Event::Deep => goto!(
                    Open { host: "b.org".to_string() }
                        / Admin { user: "bob".to_string() }
                        / Settings { tab: 2 }
                ),
            }
        }

        #[on(Public)]
        fn on_public(_event: &Event) -> Next<Self> {
            goto!(Closed)
        }
        #[on(Dashboard)]
        fn on_dashboard(_event: &Event) -> Next<Self> {
            goto!(Closed)
        }
        #[on(Settings)]
        fn on_settings(_settings: SettingsMut<'_>, _event: &Event) -> Next<Self> {
            goto!(Closed)
        }
    }
}

use site::Site;

fn main() {
    let mut machine = Machine::<Site>::new(Vec::new());
    machine.start().unwrap();
    machine.ctx_mut().clear();

    machine.send(Event::Shallow).unwrap();
    assert_eq!(machine.path().to_string(), "Open/Admin/Dashboard");
    assert_eq!(
        std::mem::take(machine.ctx_mut()),
        ["enter Open a.org", "enter Admin ada", "enter Dashboard"]
    );

    machine.send(Event::Shallow).unwrap(); // any event leaves `Dashboard`
    machine.ctx_mut().clear();

    machine.send(Event::Deep).unwrap();
    assert_eq!(machine.path().to_string(), "Open/Admin/Settings");
    assert_eq!(
        *machine.ctx(),
        ["enter Open b.org", "enter Admin bob", "enter Settings tab 2"]
    );
}
```

### Constructor form

Sometimes the new state needs data from the state that is leaving. Pass a closure as the second argument. It receives the exiting state **by value** and returns the new value:

```text
goto!(Failed, |old: Exiting![Active]| Player::Failed { track: old.track })
```

`Exiting![Active]` names the topmost state that exits, the child of the lowest common ancestor. For a compound state it is the struct itself, with its substates. For a struct-like variant, the macro generates a `<Variant>Exiting` struct. The closure returns the new value of that same level: the root enum for a transition at the top, or the substate enum of the common ancestor otherwise.

The closure runs after the exits and before the entries. That is the place for a UML transition action. Two rules:

- It runs after the handler has returned, so it cannot use what the handler borrows (its data, its `*Mut` view, its `*Fields`). Copy what you need into a local and use a `move` closure. Otherwise you get [E0046](../errors/E0046.md).
- `old` already holds the data of the exiting state, so most closures need nothing else.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Start(u32),
    Fail(String),
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
        Failed { track: u32, reason: String },
    }

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
        Playing,
    }

    #[handlers]
    impl Player {
        #[on(Idle)]
        fn on_idle(event: &Event) -> Next<Self> {
            match event {
                Event::Start(track) => goto!(Active { track: *track }),
                _ => unhandled!(),
            }
        }

        #[on(Playing)]
        fn on_playing(event: &Event) -> Next<Self> {
            match event {
                Event::Fail(reason) => {
                    let reason = reason.clone();
                    goto!(Failed, move |old: Exiting![Active]| Player::Failed {
                        track: old.track,
                        reason,
                    })
                }
                _ => bubble!(),
            }
        }

        #[on(Failed)]
        fn on_failed(_failed: FailedMut<'_>, _event: &Event) -> Next<Self> {
            goto!(Idle)
        }
    }
}

use player::Player;

fn main() {
    let mut machine = Machine::<Player>::new(());
    machine.send(Event::Start(9)).unwrap();
    machine.send(Event::Fail("disk".to_string())).unwrap();
    assert_eq!(
        *machine.state(),
        Player::Failed { track: 9, reason: "disk".to_string() }
    );
}
```

## Next

[Transition kinds](./06-transition-kinds.md) covers `history`, `external`, local transitions and `reenter!()`.
