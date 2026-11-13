# Macros

The transition macros are the return values of handlers. They are only meaningful inside a function of a `#[handlers]` block in a `#[machine]` module, which rewrites them before they expand. The definitions exported by `estado` exist so that the names resolve; one that expands anywhere else is error [`E0040`](../errors/E0040.md).

All of them are in `estado::prelude`. They can be written qualified (`estado::goto!(..)`).

| Macro | Result | Arguments |
| --- | --- | --- |
| [`goto!`](#goto) | Transition to a state | target, optionally `external`, `history` or a constructor |
| [`stay!`](#stay) | Handled, no transition | none |
| [`bubble!`](#bubble) | Hand the event to the parent state | none |
| [`unhandled!`](#unhandled) | Apply the unhandled policy | none |
| [`defer!`](#defer) | Keep the event until the configuration changes | none |
| [`reenter!`](#reenter) | Exit and enter the handler's own state again | none |
| [`Exiting!`](#constructor-form) | Type of the `old` parameter of a constructor | a state name |
| [`goto_dyn!`](#reserved-macros), [`edges!`](#reserved-macros) | Reserved | not usable yet |

Any argument given to `stay!`, `bubble!`, `unhandled!`, `defer!` or `reenter!` is [`E0043`](../errors/E0043.md).

Where each macro is allowed:

| Handler role | Allowed macros |
| --- | --- |
| `#[on]` | all of the table above |
| `#[always]`, `#[on_done]` | `goto!`, `stay!` only ([`E0045`](../errors/E0045.md)) |
| `#[entry]`, `#[exit]` | none ([`E0042`](../errors/E0042.md)) |

A macro can appear anywhere in the body: inside a `match`, a closure, a block or a nested expression. A handler can contain any number of `goto!`s, and each one is checked at compile time.

## goto!

`goto!` names a state of the machine. Every state has a unique name, so the target needs no path. An unknown name is [`E0003`](../errors/E0003.md), with a suggestion when a name is close.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Open,
    OpenWide,
    Lock(u32),
    Slam,
    Narrow,
    Rebuild,
    Close,
    Come,
    Crash(String),
}

#[estado::machine(event = Event, ctx = Vec<String>, allow(W0001, W0002))]
mod door {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Door {
        #[initial]
        Closed,
        Locked {
            code: u32,
        },
        Ajar(Ajar),
        Broken(String),
    }

    #[state]
    #[history]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Ajar {
        pub opened_by: u32,
        #[substates]
        pub swing: Swing,
    }

    #[state]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Swing {
        #[initial]
        Narrow,
        Wide,
    }

    #[handlers]
    impl Door {
        #[on(Closed)]
        fn on_closed(event: &Event) -> Next<Self> {
            match event {
                // Simple form: data for the state, if it has any.
                Event::Open => goto!(Ajar { opened_by: 1 }),
                // Path form: data for `Ajar`, then the substate to start in.
                Event::OpenWide => goto!(Ajar { opened_by: 2 } / Wide),
                // Struct-like variant.
                Event::Lock(code) => goto!(Locked { code: *code }),
                // History form: `Ajar` resumes its last substate.
                Event::Come => goto!(Ajar { opened_by: 3 }, history),
                _ => unhandled!(),
            }
        }

        #[on(Locked)]
        fn on_locked(_locked: LockedMut<'_>, event: &Event) -> Next<Self> {
            match event {
                // Constructor form: build the new state from the old one.
                Event::Slam => goto!(Broken, |old: Exiting![Locked]| {
                    Door::Broken(format!("code {}", old.code))
                }),
                // Tuple form.
                Event::Crash(reason) => goto!(Broken(reason.clone())),
                _ => unhandled!(),
            }
        }

        #[on(Narrow)]
        fn on_narrow(event: &Event) -> Next<Self> {
            match event {
                // A sibling: `Ajar` stays active.
                Event::Open => goto!(Wide),
                _ => bubble!(),
            }
        }

        #[on(Wide)]
        fn on_wide(event: &Event) -> Next<Self> {
            match event {
                // An ancestor: `Ajar` stays active, its substates restart.
                Event::Narrow => goto!(Ajar),
                // An ancestor with `external`: `Ajar` itself exits and enters.
                Event::Rebuild => goto!(Ajar { opened_by: 7 }, external),
                _ => bubble!(),
            }
        }

        #[on(Ajar)]
        fn on_ajar(event: &Event) -> Next<Self> {
            match event {
                Event::Close => goto!(Closed),
                _ => unhandled!(),
            }
        }

        #[entry(Ajar)]
        fn entry_ajar(up: AjarFields<'_>, cx: &mut Cx<Self>) {
            cx.ctx_mut().push(format!("enter Ajar {}", up.opened_by));
        }

        #[exit(Ajar)]
        fn exit_ajar(cx: &mut Cx<Self>) {
            cx.ctx_mut().push("exit Ajar".to_owned());
        }
    }
}

use door::{Door, Swing};

fn main() {
    let mut machine = Machine::<Door>::new(Vec::new());

    machine.send(Event::OpenWide).unwrap();
    assert_eq!(machine.path().to_string(), "Ajar/Wide");

    // To an ancestor: `Ajar` does not exit, its substates restart.
    machine.send(Event::Narrow).unwrap();
    assert_eq!(machine.path().to_string(), "Ajar/Narrow");
    assert_eq!(machine.ctx(), &["enter Ajar 2"]);

    // A sibling inside `Ajar` does not exit it either.
    machine.send(Event::Open).unwrap();
    assert_eq!(machine.path().to_string(), "Ajar/Wide");
    assert_eq!(machine.ctx().len(), 1);

    // `external` exits and enters `Ajar`, with new data.
    machine.send(Event::Rebuild).unwrap();
    assert_eq!(machine.ctx().len(), 3);
    assert_eq!(machine.ctx().last().map(String::as_str), Some("enter Ajar 7"));

    // Leaving records the shallow history; `history` resumes it.
    machine.send(Event::Open).unwrap();
    machine.send(Event::Close).unwrap();
    machine.send(Event::Come).unwrap();
    assert_eq!(machine.path().to_string(), "Ajar/Wide");
    assert_eq!(
        *machine.state(),
        Door::Ajar(door::Ajar { opened_by: 3, swing: Swing::Wide })
    );

    // The constructor form receives the owned fields of the state that exits.
    let mut locked = Machine::<Door>::new(Vec::new());
    locked.send(Event::Lock(4321)).unwrap();
    locked.send(Event::Slam).unwrap();
    assert_eq!(*locked.state(), Door::Broken("code 4321".to_owned()));
}
```

### Target forms

| Form | Meaning |
| --- | --- |
| `goto!(State)` | A state without data, or an ancestor whose data is already there. |
| `goto!(State { field: value, .. })` | A struct-like variant, or a `#[state]` struct. Give every field, or end with `..base` to take the rest from `base`. |
| `goto!(State(value, ..))` | A newtype or tuple variant. |
| `goto!(Outer { .. } / Inner { .. })` | A path: data for each level that is entered, outermost first. Each level must be below the previous one. |
| `goto!(Target, external)` | Exit and enter `Target`, which must be the handler's own state or one of its ancestors. |
| `goto!(Target, history)` | Enter `Target` and resume the substate it recorded. |
| `goto!(Target, \|old: Exiting![Source]\| value)` | The [constructor form](#constructor-form). |

What a level needs:

- A level that has fields needs a value for each (`E0020` otherwise). A level without `#[initial(with = ..)]` that is entered through its `#[initial]` and has fields is also `E0020`.
- The levels of a compound state below the target start in their `#[initial]` substate (or the one the path names). Regions of a parallel state not named by the path start in their `#[initial]`.
- Data for a level that does not exit or enter in this transition is `E0043`. Example: `goto!(Ajar { opened_by: 1 })` from inside `Ajar`.
- A target that is a region is `E0043`. A target in a sibling region of the handler's own state is [`E0021`](../errors/E0021.md). A `goto!` out of a `#[terminal]` state is [`E0022`](../errors/E0022.md).

### Which states exit and enter

With `H` the state of the handler and `T` the target:

| Relation | States that exit | States that enter |
| --- | --- | --- |
| `T` is `H` | `H` and what is below it | `H` and its initial chain |
| `T` is a descendant of `H` | the whole active child of `H`, with everything below it | the new child of `H` and the states down to `T` |
| `T` is an ancestor of `H` | the states below `T` | the initial chain of `T`'s substates; `T` stays active |
| `T` is an ancestor of `H`, with `external` | `T` and what is below it | `T` and its initial chain |
| Anything else | up to, not including, the least common ancestor | down to `T` |

Exits run deepest first, entries outermost first, regions in declaration order (reverse on exit). Timers and tasks of an exiting state are cancelled before the new ones start; see [effects](effects.md).

### History form

`goto!(Target, history)` needs `Target` to be a `#[state]` struct with `#[history]` (`E0043` otherwise). Give the fields of `Target` as usual. The substate comes from the history. When `Target` has no history yet, it starts in its `#[initial]` substate.

- Shallow history records which direct substate was active. `goto!(Target, history)` on a shallow history whose substates carry data is `E0090`.
- Deep history (`#[history(deep, keep)]`) records the whole substate value, nested data included. A deep history nested inside another one is `E0090`.

### Constructor form

```rust,ignore
goto!(Failed, |old: Exiting![Active]| Player::Failed { reason: old.reason })
```

The closure builds the value that replaces the state that exits. Use it to move data out of the old state, for example a field that is not `Clone`.

- `Exiting![X]` names the state `X` that exits: the child of the transition's least common ancestor on the path from the handler's state. Naming another state is `E0043`.
- The closure takes exactly one parameter, `old`. A type annotation other than `Exiting![X]` is kept as written; the macro fills in the type when you write `Exiting![X]`.
- The type of `old` depends on `X`:

  | `X` is | `old` is |
  | --- | --- |
  | A unit variant | `()` |
  | A newtype variant | the payload type `T`, by value |
  | A struct-like or tuple variant | `<X>Exiting`, with the fields as `pub` owned fields (`old.code`, or `old.0`) |
  | A compound state with `#[history(deep, keep)]` | `<Struct>Exiting`, the shared fields without the substates |

- The closure returns the new value of the child of the least common ancestor, as the enum that owns it (for example `Door::Broken(..)`, or `Link::Up` for a state of a region). The target takes no data in this form: `goto!(Failed { code: 1 }, |old| ..)` is [`E0043`](../errors/E0043.md), because the data would be ignored.
- The closure runs after the handler returned. It cannot use anything the handler borrows: the own data, the view and the `*Fields` parameters. That is [`E0046`](../errors/E0046.md). Copy what you need into a local and use a `move` closure.
- It is `E0090` when the exiting state contains a deep history, because the constructor would drop it. It is `E0043` when going to a substate of the handler's own state, because it is not known which substate exits.

## stay!

`stay!()` ends the event as handled and changes nothing. The report has `handled == true` and no transition counts.

## bubble!

`bubble!()` gives the event to the handler of the parent state. Bubbling continues up to the root. If the state has its own `#[unhandled(..)]`, that override applies at once and the parent's handler does not run. An event that bubbles out of the root is unhandled and gets the machine's [unhandled policy](attributes.md#unhandled). Use it as the catch-all arm of a substate.

## unhandled!

`unhandled!()` stops the search. The policy of the nearest `#[unhandled(..)]` on the state or an ancestor applies, or the machine's `unhandled` option when there is none:

| Policy | An external event | An internal event |
| --- | --- | --- |
| `ignore` (default) | `Ok`, with `report.ignored == Some(IgnoreReason::Unhandled)` | counted in `report.unhandled_internal` |
| `error` | `Err(SendError::Unhandled(event))`, the configuration is unchanged | counted in `report.unhandled_internal` |
| `panic` | panics, which poisons the machine | panics |

Unlike `bubble!`, `unhandled!` does not ask the ancestors for a handler. See [errors](errors.md).

## defer!

`defer!()` keeps the event in the defer queue. Deferred events are delivered again, in order, after the active configuration changes. They run after the internal events that a transition raised.

- The event is stored by value in the queue.
- The queue holds `defer_cap` events (default 8). When it is full, `defer_overflow` decides: `error` returns `Err(SendError::DeferOverflow(event))` without storing the event (the machine stays usable when it is the event you sent, and is poisoned when it is a raised or redelivered event, after the earlier microsteps of the step already ran), `drop_oldest` removes the oldest deferred event.
- A `defer!()` in a state that can never leave it is the warning [`W0003`](../errors/W0003.md).
- It is not allowed in `#[always]` or `#[on_done]` ([`E0045`](../errors/E0045.md)).

## reenter!

`reenter!()` exits the handler's state and enters it again. The data of the state stays as it is; its substates and regions restart from their `#[initial]` states. Exit and entry handlers run, timers restart with a new epoch, and a deep or shallow history is recorded. Use `goto!(Own { .. })` to give the state new data.

`reenter!()` on a parallel state restarts every region. It is not allowed in `#[always]` or `#[on_done]` ([`E0045`](../errors/E0045.md)); use `goto!(.., external)` there.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Start,
    Retry,
    Noise,
}

#[estado::machine(event = Event, ctx = Vec<&'static str>, allow(W0002))]
mod worker {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Worker {
        #[initial]
        Idle,
        Busy { attempts: u8 },
    }

    #[handlers]
    impl Worker {
        #[entry(Busy)]
        fn entry_busy(cx: &mut Cx<Self>) {
            cx.ctx_mut().push("enter Busy");
        }

        #[exit(Busy)]
        fn exit_busy(cx: &mut Cx<Self>) {
            cx.ctx_mut().push("exit Busy");
        }

        #[on(Idle)]
        fn on_idle(event: &Event) -> Next<Self> {
            match event {
                Event::Start => goto!(Busy { attempts: 1 }),
                // Not handled here: the policy decides (ignore by default).
                _ => unhandled!(),
            }
        }

        #[on(Busy)]
        fn on_busy(busy: BusyMut<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Retry => {
                    *busy.attempts += 1;
                    reenter!()
                }
                Event::Noise => stay!(),
                _ => unhandled!(),
            }
        }
    }
}

use worker::Worker;

fn main() {
    let mut machine = Machine::<Worker>::new(Vec::new());
    machine.send(Event::Start).unwrap();

    // `stay!` runs no entry or exit.
    let report = machine.send(Event::Noise).unwrap().report;
    assert!(report.handled);
    assert_eq!(report.transitions, 0);

    // `reenter!` keeps the data and runs the exit and the entry again.
    machine.send(Event::Retry).unwrap();
    assert_eq!(*machine.state(), Worker::Busy { attempts: 2 });
    assert_eq!(
        machine.ctx(),
        &["enter Busy", "exit Busy", "enter Busy"]
    );

    // `unhandled!` under the default `ignore` policy.
    let report = machine.send(Event::Start).unwrap().report;
    assert!(!report.handled);
}
```

## Reserved macros

| Macro | Status |
| --- | --- |
| `goto_dyn!(value)` | Transition to a value built at run time. Not implemented. It is exported and in the prelude, and a use inside a handler is reported as [`E0090`](../errors/E0090.md), not supported yet. Outside a handler it is [`E0040`](../errors/E0040.md). |
| `edges![A \| B]` | The return type of an `#[edges(A \| B)]` helper. Not implemented: `#[edges]` and a use of `edges!` inside a handler are `E0090`. |

The attributes `#[choice]`, `#[on_child_done]`, `#[on_resume]`, `#[invariant]`, `#[edges]` and `#[on_error]` are also not supported yet; each is `E0090`.
