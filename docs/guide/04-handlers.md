# Handlers

Handlers are the reactions of your machine. A handler is an ordinary function in the `#[handlers]` block. An attribute ties it to a state. It reads the event and returns what the machine should do next.

## `#[handlers]` and `#[on]`

Put one `#[handlers]` block on an `impl` of the root type. Inside it, `#[on(State)]` marks the handler of `State`. Each state has at most one `#[on]` ([E0005](../errors/E0005.md)).

The function name does not matter. The attribute links a handler to a state, and the parameters are recognised by their **type**, never by their name. Handlers must not be `async` ([E0060](../errors/E0060.md)). Run asynchronous work with `#[invoke]` instead, see [Timers and tasks](./15-timers.md).

A state without `#[on]` behaves as if its handler returned `bubble!()`. The exception is a state with its own `#[unhandled]`: the override applies at once and the parent's handler does not run. See [Unhandled events](./07-unhandled-events.md#unhandled-on-one-state).

## Handler parameters

All parameters are optional. When you use several, write them in this order:

| Position | Type | What it is |
| --- | --- | --- |
| 1 | `&mut T`, `TMut<'_>`, or nothing | The data of the state itself. See [State data and views](./03-state-data.md). |
| 2 | `AncestorFields<'_>` | The shared fields of an ancestor, nearest first. |
| 3 | `&mut Cx<Self>` | The context: effects, raised events, the global context. See [Context and effects](./11-context-and-effects.md). |
| 4 | `&Event` | The event. |

The return type is `Next<Self>`. A parameter of another type, or in the wrong order, is [E0006](../errors/E0006.md).

A compound state's own handler gets its `*Fields` view, never a `&mut` to the whole struct. The substates live in that struct and the dispatch code is using them.

The next example uses all four parameters in one handler. It reads the state's own data, changes the parent's shared field, emits an effect and reads the event:

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Open,
    Deposit(u32),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Notify(String),
}

#[estado::machine(event = Event, effect = Command, ctx = u32)]
mod account {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Account {
        #[initial]
        Closed,
        Open(Open),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Open {
        pub balance: u32,
        #[substates]
        pub tier: Tier,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Tier {
        #[initial]
        Basic { deposits: u32 },
        Gold,
    }

    #[handlers]
    impl Account {
        #[on(Closed)]
        fn on_closed(_event: &Event) -> Next<Self> {
            goto!(Open { balance: 0 } / Basic { deposits: 0 })
        }

        // 1: own data (`BasicMut`), 2: the parent's fields (`OpenFields`),
        // 3: the context, 4: the event.
        #[on(Basic)]
        fn on_basic(
            basic: BasicMut<'_>,
            open: OpenFields<'_>,
            cx: &mut Cx<Self>,
            event: &Event,
        ) -> Next<Self> {
            match event {
                Event::Deposit(amount) => {
                    *basic.deposits += 1;
                    *open.balance += amount;
                    *cx.ctx_mut() += 1; // the context counts all deposits
                    if *open.balance >= 100 {
                        cx.emit(Command::Notify(format!("gold after {} deposits", basic.deposits)));
                        goto!(Gold)
                    } else {
                        stay!()
                    }
                }
                _ => bubble!(),
            }
        }

        #[on(Gold)]
        fn on_gold(_event: &Event) -> Next<Self> {
            goto!(Closed)
        }
    }
}

use account::{Account, Open, Tier};

fn main() {
    let mut machine = Machine::<Account>::new(0);
    machine.send(Event::Open).unwrap();
    machine.send(Event::Deposit(60)).unwrap();
    let step = machine.send(Event::Deposit(60)).unwrap();

    assert_eq!(
        step.effects.commands().collect::<Vec<_>>(),
        [&Command::Notify("gold after 2 deposits".to_string())]
    );
    assert_eq!(*machine.ctx(), 2);
    assert_eq!(*machine.state(), Account::Open(Open { balance: 120, tier: Tier::Gold }));
}
```

Handlers are plain associated functions. Helpers that are not handlers can live in the module, outside the `#[handlers]` block, and handlers can call them. A handler may also leave early with `return goto!(..)`.

## What a handler returns

| Macro | Meaning |
| --- | --- |
| `goto!(..)` | Move to another state. See [Transitions](./05-transitions.md). |
| `stay!()` | The event is handled and nothing changes. No exit or entry runs. |
| `bubble!()` | Pass the event to the parent state's handler. |
| `unhandled!()` | This state does not handle the event, and the parents are not asked. Apply the unhandled policy. See [Unhandled events](./07-unhandled-events.md). |
| `reenter!()` | Exit and enter the handler's own state again. See [Transitions](./05-transitions.md). |
| `defer!()` | Keep the event and try it again later. See [Defer and raise](./18-defer-and-raise.md). |

These macros only work inside a `#[handlers]` function. Outside a handler they report [E0040](../errors/E0040.md). In a plain helper function of a `#[handlers]` impl (one without a role attribute) they report [E0090](../errors/E0090.md), because a helper has no state to rewrite them against: return data from the helper and call `goto!` in the handler.

A handler returns `Next<Self>`. A handler that returns `Result<Next<Self>, E>` is planned for use with `#[on_error]`, which is not supported yet, so it reports [E0061](../errors/E0061.md). Return `Next<Self>` and turn the error into a `goto!` in the body.

## How an event is dispatched

`send` starts at the **active leaf** and works upward:

1. The leaf's handler runs. A leaf without a handler counts as `bubble!()` (unless it has its own `#[unhandled]`, which applies at once).
2. After `bubble!()`, the parent's handler gets the same event, then its parent's, up to the root.
3. The first handler that answers `stay!()`, `goto!`, `reenter!()` or `defer!()` ends the search.
4. A `bubble!()` or `unhandled!()` at the root means nobody handled the event. The unhandled policy applies.

The next example logs the order. It sends events that the leaf passes up.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Ping,
    Crash,
}

#[estado::machine(event = Event, ctx = Vec<&'static str>)]
mod stack {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Stack {
        #[initial]
        Idle,
        Outer(Outer),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Outer {
        #[substates]
        pub sub: OuterSub,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum OuterSub {
        #[initial]
        Middle(Middle),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Middle {
        #[substates]
        pub sub: MiddleSub,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum MiddleSub {
        #[initial]
        Deep,
    }

    #[handlers]
    impl Stack {
        #[on(Idle)]
        fn on_idle(_event: &Event) -> Next<Self> {
            goto!(Outer)
        }

        #[on(Deep)]
        fn on_deep(cx: &mut Cx<Self>, _event: &Event) -> Next<Self> {
            cx.ctx_mut().push("Deep");
            bubble!()
        }

        #[on(Middle)]
        fn on_middle(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            cx.ctx_mut().push("Middle");
            match event {
                Event::Ping => stay!(),
                _ => bubble!(),
            }
        }

        #[on(Outer)]
        fn on_outer(cx: &mut Cx<Self>, _event: &Event) -> Next<Self> {
            cx.ctx_mut().push("Outer");
            goto!(Idle)
        }
    }
}

use stack::Stack;

fn main() {
    let mut machine = Machine::<Stack>::new(Vec::new());
    machine.send(Event::Ping).unwrap(); // Idle -> Outer/Middle/Deep
    machine.ctx_mut().clear();

    // `Middle` answers `stay!()`, so `Outer` never sees `Ping`.
    machine.send(Event::Ping).unwrap();
    assert_eq!(machine.ctx(), &["Deep", "Middle"]);
    machine.ctx_mut().clear();

    // `Crash` bubbles through `Middle` to `Outer`, which leaves.
    machine.send(Event::Crash).unwrap();
    assert_eq!(machine.ctx(), &["Deep", "Middle", "Outer"]);
    assert_eq!(*machine.state(), Stack::Idle);
}
```

A handler may change its data before it answers. Those changes stay even when the answer is `unhandled!()` or `bubble!()`. Change data only when you return `stay!()` or `goto!`, or the machine ends up with a change for an event it did not handle:

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Count,
    Other,
    Reset,
}

#[estado::machine(event = Event)]
mod sloppy {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Sloppy {
        #[initial]
        Idle,
        Busy { seen: u32 },
    }

    #[handlers]
    impl Sloppy {
        #[on(Idle)]
        fn on_idle(_event: &Event) -> Next<Self> {
            goto!(Busy { seen: 0 })
        }

        #[on(Busy)]
        fn on_busy(busy: BusyMut<'_>, event: &Event) -> Next<Self> {
            // Counts every event, even the one it then refuses.
            *busy.seen += 1;
            match event {
                Event::Count => stay!(),
                Event::Reset => goto!(Idle),
                Event::Other => unhandled!(),
            }
        }
    }
}

use sloppy::Sloppy;

fn main() {
    let mut machine = Machine::<Sloppy>::new(());
    machine.send(Event::Count).unwrap(); // starts, then Idle -> Busy
    machine.send(Event::Count).unwrap();
    let report = machine.send(Event::Other).unwrap().report;

    assert!(!report.handled);
    // The refused event still changed the data.
    assert_eq!(*machine.state(), Sloppy::Busy { seen: 2 });
}
```

## Next

[Transitions](./05-transitions.md) covers every form of `goto!`.
