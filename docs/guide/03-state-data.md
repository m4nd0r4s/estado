# State data and views

This page shows how states carry data and which views the macro generates so handlers can reach it.

## Data in states

Where a state keeps its data decides how a handler reads it.

| Declaration | A handler of that state receives |
| --- | --- |
| Unit variant: `Idle` | nothing |
| Newtype variant: `Online(Online)` with a `#[state]` struct, or `Archived(String)` | `&mut Online`, `&mut String` |
| Struct-like variant: `Running { attempts: u8 }` | a view `RunningMut<'_>` with a `&mut` per field |
| Tuple variant with several fields: `Pair(u8, u8)` | a view `PairMut<'_>`, a tuple of `&mut` |
| Compound or parallel state: `Active(Active)` | a view `ActiveFields<'_>` of the shared fields |

A newtype variant is the plainest choice when a state has data of its own. The macro cannot hand out `&mut` to a struct-like variant as a whole, so it generates a view for it instead.

This machine has one state of each kind. Every handler changes its own data in place:

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Next,
    Bump,
}

#[estado::machine(event = Event)]
mod shapes {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Shapes {
        #[initial]
        Empty,
        /// A newtype around a `#[state]` struct.
        Counting(Counter),
        /// A newtype around any type.
        Named(String),
        /// Struct-like: handlers get `PointMut`.
        Point { x: i32, y: i32 },
        /// Tuple: handlers get `PairMut`, with fields `.0` and `.1`.
        Pair(u8, u8),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Counter {
        pub count: u32,
    }

    #[handlers]
    impl Shapes {
        #[on(Empty)]
        fn on_empty(_event: &Event) -> Next<Self> {
            goto!(Counting(Counter { count: 0 }))
        }

        #[on(Counting)]
        fn on_counting(counter: &mut Counter, event: &Event) -> Next<Self> {
            match event {
                Event::Bump => {
                    counter.count += 1;
                    stay!()
                }
                Event::Next => goto!(Named("shape".to_string())),
            }
        }

        #[on(Named)]
        fn on_named(name: &mut String, event: &Event) -> Next<Self> {
            match event {
                Event::Bump => {
                    name.push('!');
                    stay!()
                }
                Event::Next => goto!(Point { x: 1, y: 2 }),
            }
        }

        #[on(Point)]
        fn on_point(point: PointMut<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Bump => {
                    *point.x += 10;
                    *point.y += 10;
                    stay!()
                }
                Event::Next => goto!(Pair(1, 2)),
            }
        }

        #[on(Pair)]
        fn on_pair(pair: PairMut<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Bump => {
                    *pair.0 += 1;
                    *pair.1 += 2;
                    stay!()
                }
                Event::Next => goto!(Empty),
            }
        }
    }
}

use shapes::{Counter, Shapes};

fn main() {
    let mut machine = Machine::<Shapes>::new(());
    machine.send(Event::Next).unwrap();
    machine.send(Event::Bump).unwrap();
    assert_eq!(*machine.state(), Shapes::Counting(Counter { count: 1 }));

    machine.send(Event::Next).unwrap();
    machine.send(Event::Bump).unwrap();
    assert_eq!(*machine.state(), Shapes::Named("shape!".to_string()));

    machine.send(Event::Next).unwrap();
    machine.send(Event::Bump).unwrap();
    assert_eq!(*machine.state(), Shapes::Point { x: 11, y: 12 });

    machine.send(Event::Next).unwrap();
    machine.send(Event::Bump).unwrap();
    assert_eq!(*machine.state(), Shapes::Pair(2, 4));
}
```

### `XFields`: the shared fields of a parent

For each compound or parallel struct, the macro generates `<Name>Fields<'a>`. It holds a `&'a mut` to every field of the struct **except** the `#[substates]` and `#[region]` fields. A handler or entry of the parent receives it. A handler of any descendant may also ask for it, which gives the substate access to the parent's data while the substate's own data is borrowed at the same time.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Begin,
    Tick,
}

#[estado::machine(event = Event)]
mod counter {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Counter {
        #[initial]
        Off,
        On(On),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct On {
        pub total: u32,
        #[substates]
        pub step: Step,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Step {
        #[initial]
        Slow { ticks: u32 },
        Fast,
    }

    #[handlers]
    impl Counter {
        #[on(Off)]
        fn on_off(_event: &Event) -> Next<Self> {
            goto!(On { total: 0 } / Slow { ticks: 0 })
        }

        // `Slow` borrows its own data and the fields of `On` together.
        #[on(Slow)]
        fn on_slow(slow: SlowMut<'_>, on: OnFields<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Tick => {
                    *slow.ticks += 1;
                    *on.total += 1;
                    if *slow.ticks == 2 { goto!(Fast) } else { stay!() }
                }
                _ => bubble!(),
            }
        }

        #[on(Fast)]
        fn on_fast(on: OnFields<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Tick => {
                    *on.total += 10;
                    stay!()
                }
                Event::Begin => goto!(Slow { ticks: 0 }),
            }
        }
    }
}

use counter::{Counter, On, Step};

fn main() {
    let mut machine = Machine::<Counter>::new(());
    for event in [Event::Begin, Event::Tick, Event::Tick, Event::Tick] {
        machine.send(event).unwrap();
    }
    assert_eq!(*machine.state(), Counter::On(On { total: 12, step: Step::Fast }));
}
```

Parameter order matters: the state's own data first, then ancestor `*Fields` from the nearest to the farthest, then `cx`, then the event. All are optional. A wrong order is [E0006](../errors/E0006.md). [Handlers](./04-handlers.md) lists the full set.

Fields keep their own visibility: a view only exposes the fields you made `pub`.

### `XMut` and `XRef`

For a struct-like or tuple variant `Running { attempts: u8 }` the macro generates `RunningMut<'a>` with one `&'a mut` field per field of the variant. Handlers, entries and exits receive it.

A variant with `#[after]` or `#[invoke]` also gets a shared view, `RunningRef<'a>`, with `&` fields. Timer and task closures receive it, because they run from a shared borrow. [Timers](./15-timers.md) and [Tasks](./16-tasks.md) show it in use.

A view of a variant keeps the visibility of its enum.

### Passing a view on with `reborrow`

A `*Fields` view is a bundle of `&mut` borrows. When you want to hand it to a helper function and still use it afterwards, call `reborrow()`. It gives a second view that borrows from the first:

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Start,
    Tick,
}

#[estado::machine(event = Event)]
mod meter {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Meter {
        #[initial]
        Off,
        On(On),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct On {
        pub total: u32,
        pub ticks: u32,
        #[substates]
        pub mode: Mode,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Mode {
        #[initial]
        Counting,
        Paused,
    }

    fn count(on: OnFields<'_>, amount: u32) {
        *on.total += amount;
        *on.ticks += 1;
    }

    #[handlers]
    impl Meter {
        #[on(Off)]
        fn on_off(_event: &Event) -> Next<Self> {
            goto!(On { total: 0, ticks: 0 })
        }

        #[on(Counting)]
        fn on_counting(mut on: OnFields<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Tick => {
                    // Two calls, one view: each gets its own reborrow.
                    count(on.reborrow(), 1);
                    count(on.reborrow(), 2);
                    stay!()
                }
                Event::Start => goto!(Paused),
            }
        }

        #[on(Paused)]
        fn on_paused(_event: &Event) -> Next<Self> {
            goto!(Counting)
        }
    }
}

use meter::{Meter, On};

fn main() {
    let mut machine = Machine::<Meter>::new(());
    machine.send(Event::Start).unwrap();
    machine.send(Event::Tick).unwrap();
    let Meter::On(On { total, ticks, .. }) = machine.state() else {
        panic!("On is active");
    };
    assert_eq!((*total, *ticks), (3, 2));
}
```

## State ids and the static graph

Each state has an id constant on the root type, named after the state in SCREAMING_SNAKE_CASE, with the suffix `_ID` (`Player::PAUSED_ID`). The ids follow the tree in pre-order from 0. They are `StateId` values that you can compare, pass to `machine.is_in(..)` and `cx.in_state(..)`, and look up in the machine's **graph**.

The graph is a `static` that the macro builds from your code: every state with its parent and kind, and every `goto!` as an edge. It is plain data. Read it through `<Player as Spec>::GRAPH`.

```rust
use estado::prelude::*;
use estado::{EdgeTarget, StateKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Play,
    Pause,
    Stop,
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

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Active {
        #[substates]
        pub sub: ActiveSub,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum ActiveSub {
        #[initial]
        Playing,
        Paused,
    }

    #[handlers]
    impl Player {
        #[on(Idle)]
        fn on_idle(_event: &Event) -> Next<Self> {
            goto!(Active)
        }

        #[on(Playing)]
        fn on_playing(_event: &Event) -> Next<Self> {
            goto!(Paused)
        }

        #[on(Paused)]
        fn on_paused(_event: &Event) -> Next<Self> {
            goto!(Playing)
        }

        #[on(Active)]
        fn on_active(_active: ActiveFields<'_>, _event: &Event) -> Next<Self> {
            goto!(Idle)
        }
    }
}

use player::Player;

fn main() {
    let graph = <Player as Spec>::GRAPH;

    // States, in pre-order.
    let names: Vec<_> = graph.states.iter().map(|state| state.name).collect();
    assert_eq!(names, ["Idle", "Active", "Playing", "Paused"]);
    assert_eq!(graph.state(Player::ACTIVE_ID).kind, StateKind::Compound);
    assert_eq!(graph.state(Player::PLAYING_ID).parent, Some(Player::ACTIVE_ID));
    assert_eq!(graph.state(Player::ACTIVE_ID).initial, Some(Player::PLAYING_ID));
    assert_eq!(graph.find("Paused").map(|state| state.id), Some(Player::PAUSED_ID));

    // One edge per `goto!`: Idle -> Active, Playing -> Paused, Paused -> Playing, Active -> Idle.
    assert_eq!(graph.edges.len(), 4);
    assert!(graph
        .edges
        .iter()
        .any(|edge| edge.from == Player::IDLE_ID && edge.to == EdgeTarget::State(Player::ACTIVE_ID)));
}
```

Edges are de-duplicated by source state, target, event hint, role and kind. Two identical `goto!` in one handler give one edge, but `goto!(T)` and `goto!(T, history)` give two.

Exporting the graph as Mermaid, DOT or JSON belongs to a later milestone and is not available yet.

## Next

[Handlers](./04-handlers.md) shows how states react to events.
