# Entry, exit and automatic transitions

A state can run code when the machine enters it or leaves it. It can also react to its own entry with `#[always]`, and a compound state can react when it completes with `#[on_done]`.

## `#[entry]` and `#[exit]`

`#[entry(State)]` runs when `State` is entered. `#[exit(State)]` runs when it is left. They take the same parameters as a handler, minus the event, and return nothing:

```text
#[entry(Active)]
fn enter_active(active: ActiveFields<'_>, cx: &mut Cx<Self>) { .. }
```

An entry or an exit can call `cx.emit` and `cx.raise`. It cannot start a transition: a `goto!` or any other transition macro there is [E0042](../errors/E0042.md). Put transition logic in a handler, or react to the entry with `#[always]`.

Each state has at most one `#[entry]` and one `#[exit]` ([E0005](../errors/E0005.md)).

## The order of exits and entries

A transition always runs in this order:

1. **Exits**, from the deepest active state upward, up to the child of the lowest common ancestor (LCA). Inside a parallel state, regions exit in **reverse** declaration order, then the parallel state itself.
2. **Cancellations** of timers and tasks of the states that exited.
3. History is recorded for compound states with `#[history]` that exited.
4. The constructor of `goto!(T, |old| ..)` runs, if there is one.
5. The new value is written.
6. **Entries**, from the child of the LCA down to the leaf. Inside a parallel state, the state first, then its regions in declaration order.
7. Timers and tasks of the states that entered are scheduled.
8. `#[always]` handlers of the states that entered are queued as the next microstep.

The example logs every exit and entry. Each `send` below uses a different row of the table in [Transitions](./06-transition-kinds.md#history-and-external).

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Open,
    Switch,
    ToAncestor,
    Restart,
    Close,
}

#[estado::machine(event = Event, ctx = Vec<&'static str>)]
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
        #[entry(Closed)]
        fn enter_closed(cx: &mut Cx<Self>) {
            cx.ctx_mut().push("enter Closed");
        }
        #[exit(Closed)]
        fn exit_closed(cx: &mut Cx<Self>) {
            cx.ctx_mut().push("exit Closed");
        }
        #[entry(Open)]
        fn enter_open(_open: OpenFields<'_>, cx: &mut Cx<Self>) {
            cx.ctx_mut().push("enter Open");
        }
        #[exit(Open)]
        fn exit_open(_open: OpenFields<'_>, cx: &mut Cx<Self>) {
            cx.ctx_mut().push("exit Open");
        }
        #[entry(Insert)]
        fn enter_insert(cx: &mut Cx<Self>) {
            cx.ctx_mut().push("enter Insert");
        }
        #[exit(Insert)]
        fn exit_insert(cx: &mut Cx<Self>) {
            cx.ctx_mut().push("exit Insert");
        }
        #[entry(Command)]
        fn enter_command(cx: &mut Cx<Self>) {
            cx.ctx_mut().push("enter Command");
        }
        #[exit(Command)]
        fn exit_command(cx: &mut Cx<Self>) {
            cx.ctx_mut().push("exit Command");
        }

        #[on(Closed)]
        fn on_closed(_event: &Event) -> Next<Self> {
            goto!(Open { edits: 0 })
        }

        #[on(Insert)]
        fn on_insert(event: &Event) -> Next<Self> {
            match event {
                Event::Switch => goto!(Command),
                Event::ToAncestor => goto!(Open),
                _ => bubble!(),
            }
        }

        #[on(Command)]
        fn on_command(event: &Event) -> Next<Self> {
            match event {
                Event::ToAncestor => goto!(Open { edits: 100 }, external),
                _ => bubble!(),
            }
        }

        #[on(Open)]
        fn on_open(event: &Event) -> Next<Self> {
            match event {
                Event::Restart => reenter!(),
                Event::Close => goto!(Closed),
                _ => unhandled!(),
            }
        }
    }
}

use editor::Editor;

fn run(machine: &mut Machine<Editor>, event: Event) -> Vec<&'static str> {
    machine.send(event).unwrap();
    std::mem::take(machine.ctx_mut())
}

fn main() {
    let mut machine = Machine::<Editor>::new(Vec::new());
    machine.start().unwrap();
    machine.ctx_mut().clear();

    // Entering a compound state enters its initial substate.
    assert_eq!(
        run(&mut machine, Event::Open),
        ["exit Closed", "enter Open", "enter Insert"]
    );

    // Siblings: only the two substates change.
    assert_eq!(run(&mut machine, Event::Switch), ["exit Insert", "enter Command"]);

    // `goto!(Open, external)` leaves and enters `Open` itself.
    assert_eq!(
        run(&mut machine, Event::ToAncestor),
        ["exit Command", "exit Open", "enter Open", "enter Insert"]
    );

    // `goto!(Open)` from a substate keeps `Open` and restarts its substates.
    assert_eq!(
        run(&mut machine, Event::ToAncestor),
        ["exit Insert", "enter Insert"]
    );

    // `reenter!()` in `Open` exits the substates and `Open`, then enters them again.
    assert_eq!(
        run(&mut machine, Event::Restart),
        ["exit Insert", "exit Open", "enter Open", "enter Insert"]
    );

    // Leaving for a top-level state exits from the leaf upward.
    assert_eq!(
        run(&mut machine, Event::Close),
        ["exit Insert", "exit Open", "enter Closed"]
    );
}
```

When an entry or exit raises or emits effects, they appear in the order the code ran. Timers and tasks follow the rule above: cancellations come after every exit and before the entries, scheduling after every entry. See [Timers and tasks](./15-timers.md).

## Entries and exits with data

An entry or an exit receives the data of its state the same way a handler does: `&mut T` for a newtype variant, a `*Mut` view for a struct-like variant, a `*Fields` view for a compound state, and the `*Fields` of ancestors after that. An entry may change the data, which makes it a good place to initialise it. An exit sees the data a last time before the state is dropped.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Login,
    Logout,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Audit(String),
}

#[estado::machine(event = Event, effect = Command, ctx = Vec<String>)]
mod session {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Session {
        #[initial]
        Anonymous,
        Active(Active),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Active {
        pub user: String,
        pub actions: u32,
        #[substates]
        pub view: View,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum View {
        #[initial]
        Home { visits: u32 },
        Profile,
    }

    #[handlers]
    impl Session {
        // The entry of a compound state gets its `*Fields`.
        #[entry(Active)]
        fn enter_active(active: ActiveFields<'_>, cx: &mut Cx<Self>) {
            *active.actions = 1; // initialise
            cx.emit(Command::Audit(format!("login {}", active.user)));
        }

        #[exit(Active)]
        fn exit_active(active: ActiveFields<'_>, cx: &mut Cx<Self>) {
            cx.emit(Command::Audit(format!("logout {} after {} actions", active.user, active.actions)));
        }

        // A struct-like substate gets its view, then the parent's fields.
        #[entry(Home)]
        fn enter_home(home: HomeMut<'_>, active: ActiveFields<'_>, cx: &mut Cx<Self>) {
            *home.visits += 1;
            *active.actions += 1;
            cx.ctx_mut().push(format!("home visit {}", home.visits));
        }

        #[on(Anonymous)]
        fn on_anonymous(_event: &Event) -> Next<Self> {
            goto!(Active { user: "ada".to_string(), actions: 0 } / Home { visits: 0 })
        }

        #[on(Home)]
        fn on_home(_event: &Event) -> Next<Self> {
            goto!(Profile)
        }

        #[on(Profile)]
        fn on_profile(_event: &Event) -> Next<Self> {
            goto!(Anonymous)
        }
    }
}

use session::Session;

fn main() {
    let mut machine = Machine::<Session>::new(Vec::new());

    let step = machine.send(Event::Login).unwrap();
    assert_eq!(
        step.effects.commands().collect::<Vec<_>>(),
        [&Command::Audit("login ada".to_string())]
    );
    assert_eq!(machine.ctx(), &["home visit 1"]);

    // `actions`: 1 from the entry of `Active`, +1 from the entry of `Home`.
    machine.send(Event::Logout).unwrap(); // Home -> Profile
    let step = machine.send(Event::Logout).unwrap(); // Profile -> Anonymous
    assert_eq!(
        step.effects.commands().collect::<Vec<_>>(),
        [&Command::Audit("logout ada after 2 actions".to_string())]
    );
}
```

## Entries on start

`start()` (or the first `send`) runs the entries of the **initial configuration**, from the root down: the entry of the initial state, then the entries of its initial substates. This is the one case where entries run without a transition.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Go,
}

#[estado::machine(event = Event, ctx = Vec<String>)]
mod boot {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Boot {
        #[initial]
        Starting(Starting),
        Running,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Starting {
        #[substates]
        pub step: Step,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Step {
        #[initial]
        Hardware,
        Software,
    }

    #[handlers]
    impl Boot {
        #[entry(Starting)]
        fn enter_starting(cx: &mut Cx<Self>) {
            cx.ctx_mut().push("enter Starting".to_string());
        }
        #[entry(Hardware)]
        fn enter_hardware(cx: &mut Cx<Self>) {
            cx.ctx_mut().push("enter Hardware".to_string());
        }

        #[on(Hardware)]
        fn on_hardware(_event: &Event) -> Next<Self> {
            goto!(Software)
        }
        #[on(Software)]
        fn on_software(_event: &Event) -> Next<Self> {
            goto!(Running)
        }
        #[on(Running)]
        fn on_running(_event: &Event) -> Next<Self> {
            goto!(Starting)
        }
    }
}

use boot::Boot;

fn main() {
    let mut machine = Machine::<Boot>::new(Vec::new());
    assert!(machine.ctx().is_empty()); // `new` runs no entry

    machine.start().unwrap();
    assert_eq!(machine.ctx(), &["enter Starting", "enter Hardware"]);

    // A second `start` does nothing.
    machine.start().unwrap();
    assert_eq!(machine.ctx().len(), 2);
}
```

## Next

[Always](./09-always.md) covers automatic transitions.
