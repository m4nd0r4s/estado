# Handlers

A handler is an associated function inside a `#[handlers] impl Root { .. }` block. A role attribute (`#[on]`, `#[entry]`, `#[exit]`, `#[always]` or `#[on_done]`) names the state it belongs to. The macro recognizes each parameter by its type, so you write only the parameters you need.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Play(u32),
    Tick,
    Pause,
    Resume,
    Finish,
    Stop,
}

#[estado::machine(event = Event, ctx = Vec<String>)]
mod player {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Player {
        #[initial]
        Idle,
        Active(Active),
        Stopped(u32),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Active {
        pub track: u32,
        #[substates]
        pub stage: Stage,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Stage {
        #[initial]
        Playing { position: u32 },
        Paused(u32),
        #[terminal]
        Finished(u32),
    }

    #[handlers]
    impl Player {
        // No parameters are required. This handler takes only the event.
        #[on(Idle)]
        fn on_idle(event: &Event) -> Next<Self> {
            match event {
                Event::Play(track) => goto!(Active { track: *track } / Playing { position: 0 }),
                _ => unhandled!(),
            }
        }

        // Order: own data, ancestors' `*Fields`, `cx`, event.
        #[on(Playing)]
        fn on_playing(
            playing: PlayingMut<'_>,
            up: ActiveFields<'_>,
            cx: &mut Cx<Self>,
            event: &Event,
        ) -> Next<Self> {
            cx.ctx_mut()
                .push(format!("track {} at {}", up.track, playing.position));
            match event {
                Event::Tick => {
                    *playing.position += 1;
                    stay!()
                }
                Event::Pause => goto!(Paused(*playing.position)),
                Event::Finish => goto!(Finished(*playing.position)),
                _ => bubble!(),
            }
        }

        // A newtype variant: own data is `&mut u32`.
        #[on(Paused)]
        fn on_paused(position: &mut u32, event: &Event) -> Next<Self> {
            match event {
                Event::Resume => goto!(Playing { position: *position }),
                _ => bubble!(),
            }
        }

        // A compound state's own data is its `*Fields` view.
        #[on(Active)]
        fn on_active(up: ActiveFields<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Stop => goto!(Stopped(*up.track)),
                _ => unhandled!(),
            }
        }

        // Entries and exits take the same parameters except the event.
        #[entry(Paused)]
        fn entry_paused(position: &mut u32, cx: &mut Cx<Self>) {
            cx.ctx_mut().push(format!("paused at {position}"));
        }

        #[exit(Active)]
        fn exit_active(up: ActiveFields<'_>, cx: &mut Cx<Self>) {
            cx.ctx_mut().push(format!("exit track {}", up.track));
        }

        // `#[on_done]` also accepts `&Output`, the payload of the final substate.
        #[on_done(Active)]
        fn active_done(up: ActiveFields<'_>, total: &u32, cx: &mut Cx<Self>) -> Next<Self> {
            cx.ctx_mut()
                .push(format!("track {} done after {total}", up.track));
            goto!(Stopped(*total))
        }
    }
}

use player::Player;

fn main() {
    let mut machine = Machine::<Player>::new(Vec::new());
    machine.send(Event::Play(7)).unwrap();
    machine.send(Event::Tick).unwrap();
    machine.send(Event::Tick).unwrap();
    machine.send(Event::Finish).unwrap();

    // `Finished` completed `Active`, whose `#[on_done]` moved on to `Stopped`.
    assert_eq!(*machine.state(), Player::Stopped(2));
    assert_eq!(machine.ctx().last().map(String::as_str), Some("exit track 7"));
}
```

## Common rules

- A handler is an associated function without a `self` receiver. It must not be `async` (`E0060`).
- A role attribute takes exactly one state name: `#[on(Playing)]`. A function can have only one role (`E0005`). A state has at most one function per role (`E0005`). An unknown state name is `E0004`.
- A function without a role attribute is a plain helper. The macro does not rewrite it. A helper cannot use the transition macros: they are rewritten against the state of their handler, so `goto!` in a helper is [`E0090`](../errors/E0090.md) (not supported yet). Return a value and let a handler call `goto!`.
- All parameters are optional and recognized by type, in the order of the table below. An unknown, repeated or misplaced parameter is `E0006`.
- `cx` is always `&mut Cx<Self>`. A shared `&Cx<Self>` is `E0006`.
- The macro adds the effect sink as a hidden generic parameter and completes `Cx<Self>`. You never write it.
- Handlers of regions go on the states inside the region. A handler on the region itself (`#[on(region_name)]`) is not supported yet (`E0090`).

## Parameters

Parameters appear in this order. Each kind appears at most once, except `*Fields`, which can appear once per ancestor, nearest ancestor first.

| # | Parameter | Type | Meaning |
| --- | --- | --- | --- |
| 1 | Own data | `&mut T` or `&T`; `<Variant>Mut<'_>`; `<Struct>Fields<'_>` | The data of the state the handler belongs to. See [own data](#own-data). |
| 2 | Ancestors | `<Struct>Fields<'_>` | The shared fields of an ancestor, nearest first. |
| 3 | Output | `&T` | Only in `#[on_done]`: the payload of a `#[terminal]` substate. |
| 4 | Context | `&mut Cx<Self>` | See [context](context.md). |
| 5 | Event | `&Event` | Only in `#[on]`. The type must be the machine's `event` type. |

### Own data

What the first parameter looks like depends on the shape of the state:

| The state is | Own data parameter |
| --- | --- |
| A unit variant (`Idle`) | none |
| A newtype variant of a non-composite type (`Stopped(u32)`, `Online(Online)`) | `&mut T` or `&T`, with `T` the payload type |
| A struct-like or tuple variant (`Playing { position: u32 }`) | `<Variant>Mut<'_>` |
| A compound or parallel state (`Active(Active)`) | `<Struct>Fields<'_>` |

A handler never takes `&mut Active` for a composite struct, because its substates are borrowed separately (`E0006`). A handler can take the view of its own state only; the view of another state is `E0006`.

### Ancestors

A handler of a substate can read and write the shared fields of every ancestor through `<Struct>Fields<'_>`. List them nearest first:

```rust,ignore
#[on(Cruising)]
fn cruising(
    cruising: CruisingMut<'_>,   // own data
    flight: FlightFields<'_>,    // parent
    mission: MissionFields<'_>,  // grandparent
    event: &Event,
) -> Next<Self> { /* .. */ }
```

Asking for the view of a state that is not an ancestor is `E0006`.

## Roles

| Role | Runs | Event parameter | Returns | Allowed macros |
| --- | --- | --- | --- | --- |
| `#[on(S)]` | When an event reaches `S`: `S` is the active leaf, or an event bubbled up to it | yes | `Next<Self>` | `goto!`, `stay!`, `bubble!`, `unhandled!`, `defer!`, `reenter!` |
| `#[entry(S)]` | When `S` is entered | no | nothing | none (`E0042`) |
| `#[exit(S)]` | When `S` is exited | no | nothing | none (`E0042`) |
| `#[always(S)]` | After `S` is entered, as its own microstep; once per entry | no | `Next<Self>` | `goto!`, `stay!` (`E0045` for the others) |
| `#[on_done(S)]` | When a final substate completes `S` | no | `Next<Self>` | `goto!`, `stay!` (`E0045` for the others) |

- `#[on_done(S)]` needs a compound or parallel state (`E0044`).
- Entries and exits can call `cx.emit` and `cx.raise`, but not transition.
- A handler that returns `Result<Next<Self>, E>` is `E0061`: `#[on_error]` is not supported yet.
- `#[unhandled(policy)]` is not a handler. It is a [state attribute](attributes.md#unhandled).

## Return type

Handlers of `#[on]`, `#[always]` and `#[on_done]` return `Next<Self>`. Use the [transition macros](macros.md) to build the value:

| Macro | `Next` variant | Meaning |
| --- | --- | --- |
| `stay!()` | `Stay` | Handled; no transition. |
| `goto!(..)` | `Goto(edge)` | Transition to a state. |
| `bubble!()` | `Bubble` | Pass the event to the parent state's handler. |
| `unhandled!()` | `Unhandled` | Nobody handles it; apply the unhandled policy. |
| `defer!()` | `Defer` | Keep the event until the configuration changes. |
| `reenter!()` | `Reenter` | Exit and enter the handler's own state again. |

The macro rewrites the return type to carry a generated edge type. You still write `Next<Self>`.

## Generated view types

The macro writes these types next to your own, in the machine module. They keep the visibility of the enum or struct they come from.

| Type | For | Contents |
| --- | --- | --- |
| `<Struct>Fields<'a>` | Every compound or parallel `#[state]` struct | `&'a mut` to each field except the `#[substates]` and `#[region]` fields. Fields keep their own visibility. Has `reborrow(&mut self) -> <Struct>Fields<'_>`. |
| `<Variant>Mut<'a>` | Every struct-like or tuple variant | `pub` `&'a mut` to each field: by name for a struct-like variant, `.0`, `.1`, .. for a tuple variant. |
| `<Variant>Exiting` | A struct-like or tuple variant that a constructor `goto!` receives | The owned fields. Generated only when used. See [`goto!`](macros.md#constructor-form). |

The name of a view drops a raw-identifier prefix: the variant `r#type` has the view `typeMut`.

`reborrow` lets a handler pass the fields to a helper and keep using them:

```rust,ignore
fn bump(up: ActiveFields<'_>) { *up.rounds += 1; }

#[on(Playing)]
fn playing(mut up: ActiveFields<'_>, event: &Event) -> Next<Self> {
    bump(up.reborrow());
    *up.rounds += 1;
    stay!()
}
```

## State id constants

The macro adds one `StateId` constant per state and per region to the root type, in SCREAMING_SNAKE_CASE with the suffix `_ID`: `Player::PLAYING_ID`, `Player::PAUSED_ID`, and `Conn::KEEP_ALIVE_ID` for a region called `keep_alive`. See [context](context.md#state-ids). Two states whose names give the same constant (`Link` and a region `link`) are [`E0012`](../errors/E0012.md). States named `A` or `IO` are fine.
