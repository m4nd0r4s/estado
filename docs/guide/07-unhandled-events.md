# Unhandled events

An event is **unhandled** when no state wants it. This page explains the two ways a handler can say so, the policies that decide what happens next, and how to override a policy for one state.

## `bubble!()` and `unhandled!()`

Both macros say "this handler does not deal with the event", but they differ in what comes next:

- `bubble!()` passes the event **up** to the parent's handler. The parent may handle it. At the root, `bubble!()` becomes `unhandled!()`.
- `unhandled!()` stops the search right there. The parent's handler does **not** run. The unhandled policy applies.

A state without an `#[on]` handler counts as `bubble!()`, unless the state has its own `#[unhandled]` (see [below](#unhandled-on-one-state)). In that case the override applies at once and the parent's handler does **not** run.

The next example sends the same event to a leaf that bubbles and to a leaf that says unhandled. The parent can handle `Ping`, but only the bubbling leaf lets it:

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Enter,
    Ping,
}

#[estado::machine(event = Event, ctx = Vec<String>)]
mod tree {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Tree {
        #[initial]
        Idle,
        Outer(Outer),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Outer {
        #[substates]
        pub leaf: Leaf,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Leaf {
        #[initial]
        Passes,
        Refuses,
    }

    #[handlers]
    impl Tree {
        #[on(Idle)]
        fn on_idle(_event: &Event) -> Next<Self> {
            goto!(Outer)
        }

        #[on(Passes)]
        fn on_passes(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            cx.ctx_mut().push("Passes bubbles".to_string());
            match event {
                Event::Enter => goto!(Refuses),
                _ => bubble!(),
            }
        }

        #[on(Refuses)]
        fn on_refuses(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            cx.ctx_mut().push("Refuses says unhandled".to_string());
            match event {
                Event::Enter => goto!(Passes),
                _ => unhandled!(),
            }
        }

        #[on(Outer)]
        fn on_outer(cx: &mut Cx<Self>, _event: &Event) -> Next<Self> {
            cx.ctx_mut().push("Outer handles".to_string());
            goto!(Idle)
        }
    }
}

use tree::Tree;

fn main() {
    let mut machine = Machine::<Tree>::new(Vec::new());
    machine.send(Event::Enter).unwrap(); // Idle -> Outer/Passes
    machine.ctx_mut().clear();

    // `Passes` bubbles: the parent handles the event.
    machine.send(Event::Ping).unwrap();
    assert_eq!(machine.ctx(), &["Passes bubbles", "Outer handles"]);
    assert_eq!(*machine.state(), Tree::Idle);

    // `Refuses` says unhandled: `Outer` never sees it.
    machine.send(Event::Enter).unwrap();
    machine.send(Event::Enter).unwrap(); // Passes -> Refuses
    machine.ctx_mut().clear();
    let report = machine.send(Event::Ping).unwrap().report;
    assert_eq!(machine.ctx(), &["Refuses says unhandled"]);
    assert!(!report.handled);
}
```

## Policies

When an event is unhandled, a **policy** decides what happens:

| Policy | An external event (`send`) | An internal or redelivered event |
| --- | --- | --- |
| `ignore` (default) | `Ok`, with `report.ignored = Some(IgnoreReason::Unhandled)` | counted in `report.unhandled_internal` |
| `error` | `Err(SendError::Unhandled(event))`. The configuration is unchanged. | counted in `report.unhandled_internal` |
| `panic` | panics, which poisons the machine | panics |

An internal event is one that came from `cx.raise` or from the defer queue. Under `ignore` and `error` they never make the run fail. Under `panic` they panic too, and the panic poisons the machine.

The behaviour is the same in debug and release builds.

Choose the default for the whole machine with `unhandled = ...`:

```rust
use estado::prelude::*;
use estado::IgnoreReason;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Go,
    Noise,
}

#[estado::machine(event = Event)]
mod quiet {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Quiet {
        #[initial]
        Waiting,
        Going,
    }

    #[handlers]
    impl Quiet {
        #[on(Waiting)]
        fn on_waiting(event: &Event) -> Next<Self> {
            match event {
                Event::Go => goto!(Going),
                _ => unhandled!(),
            }
        }

        #[on(Going)]
        fn on_going(_event: &Event) -> Next<Self> {
            goto!(Waiting)
        }
    }
}

#[estado::machine(event = Event, unhandled = error)]
mod loud {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Loud {
        #[initial]
        Waiting,
        Going,
    }

    #[handlers]
    impl Loud {
        #[on(Waiting)]
        fn on_waiting(event: &Event) -> Next<Self> {
            match event {
                Event::Go => goto!(Going),
                _ => unhandled!(),
            }
        }

        #[on(Going)]
        fn on_going(_event: &Event) -> Next<Self> {
            goto!(Waiting)
        }
    }
}

fn main() {
    // `ignore`: `Ok`, and the report says why nothing happened.
    let mut quiet = Machine::<quiet::Quiet>::new(());
    let report = quiet.send(Event::Noise).unwrap().report;
    assert!(!report.handled);
    assert_eq!(report.ignored, Some(IgnoreReason::Unhandled));

    // `error`: the event comes back.
    let mut loud = Machine::<loud::Loud>::new(());
    let error = loud.send(Event::Noise).unwrap_err();
    assert_eq!(error, SendError::Unhandled(Event::Noise));
}
```

A machine that reached a root `#[terminal]` state ignores every later event, whatever the policy. See [States](./02-states.md#terminal-states).

## `#[unhandled]` on one state

`#[unhandled(policy)]` overrides the policy for one state. Put it on a variant or on a `#[state]` struct. It applies when an event leaves that state unhandled, through `unhandled!()` or through `bubble!()`. A state's **own** override applies at once, also for `bubble!()`: the parent's handler does **not** run. Only a state without its own `#[unhandled]` hands a bubbling event to the parent's handler.

The search for an override starts at the state where the event was left and goes **up through its ancestors**, so a compound state can set the policy for everything below it. The nearest override wins. Without one, the machine policy applies.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Edit,
    Poke,
    Climb,
}

#[estado::machine(event = Event)]
mod doc {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Doc {
        #[initial]
        Idle,
        Editing(Editing),
    }

    // Every state below `Editing` reports unhandled events as errors.
    #[state]
    #[unhandled(error)]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Editing {
        pub dirty: bool,
        #[substates]
        pub mode: Mode,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Mode {
        #[initial]
        Typing,
        Selecting,
    }

    #[handlers]
    impl Doc {
        #[on(Idle)]
        fn on_idle(event: &Event) -> Next<Self> {
            match event {
                Event::Edit => goto!(Editing { dirty: false }),
                _ => unhandled!(),
            }
        }

        // The leaf says `unhandled!()`: the override of `Editing` applies.
        #[on(Typing)]
        fn on_typing(event: &Event) -> Next<Self> {
            match event {
                Event::Edit => goto!(Selecting),
                Event::Climb => bubble!(),
                _ => unhandled!(),
            }
        }

        #[on(Selecting)]
        fn on_selecting(_event: &Event) -> Next<Self> {
            goto!(Idle)
        }
    }
}

use doc::Doc;

fn main() {
    let mut machine = Machine::<Doc>::new(());

    // `Idle` has no override: the machine policy, `ignore`, applies.
    assert!(machine.send(Event::Poke).is_ok());

    machine.send(Event::Edit).unwrap();
    assert_eq!(machine.path().to_string(), "Editing/Typing");

    // Both `unhandled!()` and `bubble!()` reach the override of `Editing`.
    let result = machine.send(Event::Poke);
    assert!(matches!(result, Err(SendError::Unhandled(Event::Poke))));
    let result = machine.send(Event::Climb);
    assert!(matches!(result, Err(SendError::Unhandled(Event::Climb))));

    // The machine is not poisoned and the configuration is unchanged.
    assert_eq!(machine.path().to_string(), "Editing/Typing");
}
```

A variant can also carry the attribute. `#[unhandled(ignore)]` on a state in a machine with `unhandled = error` makes that state forgiving.

When a variant and the `#[state]` struct it holds both carry `#[unhandled]`, the struct's attribute wins.

### `panic`

`#[unhandled(panic)]` on a state, or `unhandled = panic` on the machine, makes an unhandled event a bug that stops the program. A panic in the middle of `send` poisons the machine. See [Errors and safety](./21-errors-and-safety.md).

```rust
use std::panic::{AssertUnwindSafe, catch_unwind};

use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Go,
    Surprise,
}

#[estado::machine(event = Event)]
mod strict {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Strict {
        #[initial]
        Waiting,
        #[unhandled(panic)]
        Running,
    }

    #[handlers]
    impl Strict {
        #[on(Waiting)]
        fn on_waiting(event: &Event) -> Next<Self> {
            match event {
                Event::Go => goto!(Running),
                _ => unhandled!(),
            }
        }

        #[on(Running)]
        fn on_running(event: &Event) -> Next<Self> {
            match event {
                Event::Go => goto!(Waiting),
                _ => unhandled!(),
            }
        }
    }
}

use strict::Strict;

fn main() {
    let mut machine = Machine::<Strict>::new(());

    // `Waiting` uses the default policy.
    assert!(machine.send(Event::Surprise).is_ok());

    machine.send(Event::Go).unwrap();
    let outcome = catch_unwind(AssertUnwindSafe(|| machine.send(Event::Surprise)));
    assert!(outcome.is_err());
    assert!(machine.is_poisoned());
}
```

### Overrides inside regions

Inside a parallel state, an override on a state of a region stops the event there. The other regions still look at it, but the handler of the parallel state does not run. The policy applies only if no sibling region handles or defers the event: when another region handles it, the event counts as handled and the override has no effect. See [Regions](./13-regions.md).

## Next

[Entry and exit](./08-entry-exit.md) covers the code that runs when states change.
