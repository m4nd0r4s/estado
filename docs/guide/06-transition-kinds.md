# Transition kinds

This page explains which states exit and enter for each kind of transition: local, external, history and re-entry.

## `history` and `external`

- `goto!(Active { .. }, history)` enters the substate that `Active` recorded when it last exited. See [History](./17-history.md).
- `goto!(Active { .. }, external)` makes a target that is an ancestor of the handler (or the handler's state itself) exit and enter again. Without `external`, a transition to an ancestor keeps the ancestor and only restarts its substates.

Which states exit and enter depends on the **lowest common ancestor** (LCA) of the handler's state and the target:

| Form | Exits | Enters |
| --- | --- | --- |
| Target outside the handler's subtree | the active leaf up to the child of the LCA | the child of the LCA down to the target, then the target's initials |
| Target is a descendant of the handler's state | the active leaf up to the child of the handler's state | down to the target, then its initials |
| Target is an ancestor of the handler's state | the active leaf up to the child of the target | the target's initial (or history) |
| `goto!(T, external)`, `T` an ancestor of or equal to the handler's state | the active leaf up to `T`, inclusive | `T` and its initials |
| `goto!(handler's state)` or `reenter!()` | the active leaf up to the handler's state, inclusive | the handler's state and its initials |

[Entry and exit](./08-entry-exit.md) runs these rows and prints the order.

A malformed `goto!` is [E0043](../errors/E0043.md).

## Not supported yet

Some forms of the macro language are planned and report [E0090](../errors/E0090.md) when you use them: `#[choice]` pseudostates, `#[edges]` helpers (and `edges!`), `goto_dyn!` (a target decided at run time), `#[on_error]` and `#[invariant]`. A handler that returns `Result` fails with another code today, [E0061](../errors/E0061.md). Express the same logic with a `match` in a handler and a `goto!` for each case.

## `reenter!()`

`reenter!()` exits the handler's own state, with everything below it, and enters it again. The state keeps its own data and its substates restart in their initials. It is the same as `goto!` to the handler's own state, without the need to write the data again.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Faster,
    Restart,
}

#[estado::machine(event = Event, ctx = Vec<String>)]
mod playback {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Playback {
        #[initial]
        Idle,
        Playing(Playing),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Playing {
        pub speed: u8,
        #[substates]
        pub mode: Mode,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Mode {
        #[initial]
        Normal,
        Boosted,
    }

    #[handlers]
    impl Playback {
        #[entry(Playing)]
        fn enter_playing(_playing: PlayingFields<'_>, cx: &mut Cx<Self>) {
            cx.ctx_mut().push("enter Playing".to_string());
        }
        #[exit(Playing)]
        fn exit_playing(_playing: PlayingFields<'_>, cx: &mut Cx<Self>) {
            cx.ctx_mut().push("exit Playing".to_string());
        }
        #[entry(Normal)]
        fn enter_normal(cx: &mut Cx<Self>) {
            cx.ctx_mut().push("enter Normal".to_string());
        }
        #[exit(Boosted)]
        fn exit_boosted(cx: &mut Cx<Self>) {
            cx.ctx_mut().push("exit Boosted".to_string());
        }

        #[on(Idle)]
        fn on_idle(_event: &Event) -> Next<Self> {
            goto!(Playing { speed: 1 })
        }

        #[on(Normal)]
        fn on_normal(event: &Event) -> Next<Self> {
            match event {
                Event::Faster => goto!(Boosted),
                _ => bubble!(),
            }
        }

        #[on(Boosted)]
        fn on_boosted(event: &Event) -> Next<Self> {
            match event {
                Event::Faster => goto!(Normal),
                _ => bubble!(),
            }
        }

        #[on(Playing)]
        fn on_playing(playing: PlayingFields<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Faster => {
                    *playing.speed += 1;
                    stay!()
                }
                Event::Restart => reenter!(),
            }
        }
    }
}

use playback::{Mode, Playback, Playing};

fn main() {
    let mut machine = Machine::<Playback>::new(Vec::new());
    machine.send(Event::Restart).unwrap(); // Idle -> Playing
    machine.send(Event::Faster).unwrap(); // Normal -> Boosted
    machine.ctx_mut().clear();

    // Restart in `Playing`: the substate and `Playing` exit, then both enter again.
    machine.send(Event::Restart).unwrap();
    assert_eq!(
        machine.ctx(),
        &["exit Boosted", "exit Playing", "enter Playing", "enter Normal"]
    );
    // The speed survives, the substate went back to its initial.
    assert_eq!(
        *machine.state(),
        Playback::Playing(Playing { speed: 1, mode: Mode::Normal })
    );
}
```

## Next

[Unhandled events](./07-unhandled-events.md) covers `bubble!()`, `unhandled!()` and the unhandled policies.
