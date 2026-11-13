# Regions

A **parallel state** is active in several places at once. Use it when parts of your system change independently: a playback state and a volume state, a connection and its keepalive. Each part is a **region**.

You declare a parallel state as a `#[state]` struct with two or more `#[region]` fields. Each region field is a `#[state]` enum with its own `#[initial]`. Entering the parallel state enters every region, each in its initial state. The state may also carry shared fields, like a compound state does.

A struct with a single region is an error ([E0014](../errors/E0014.md)), and a struct cannot mix `#[region]` and `#[substates]` fields ([E0013](../errors/E0013.md)).

## Declaring regions and the order of entries and exits

The active configuration of a parallel state has one branch per region. `path()` prints it as `Parent/Parallel/region:State,region:State`.

The order of the `#[region]` fields in the struct is the **declaration order**. It decides everything that follows:

- On entry, the parallel state enters first, then its regions in declaration order.
- On exit, the regions exit in **reverse** declaration order, then the parallel state.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Power,
    Flip,
}

#[estado::machine(event = Event, ctx = Vec<String>)]
mod panel {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Panel {
        #[initial]
        Off,
        On(On),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct On {
        pub volume: u8,
        #[region]
        pub light: Light,
        #[region]
        pub sound: Sound,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Light {
        #[initial]
        Dim,
        Bright,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Sound {
        #[initial]
        Mute,
        Loud,
    }

    #[handlers]
    impl Panel {
        #[entry(On)]
        fn enter_on(on: OnFields<'_>, cx: &mut Cx<Self>) {
            cx.ctx_mut().push(format!("enter On (volume {})", on.volume));
        }
        #[exit(On)]
        fn exit_on(_on: OnFields<'_>, cx: &mut Cx<Self>) {
            cx.ctx_mut().push("exit On".to_string());
        }
        #[entry(Dim)]
        fn enter_dim(cx: &mut Cx<Self>) {
            cx.ctx_mut().push("enter Dim".to_string());
        }
        #[exit(Dim)]
        fn exit_dim(cx: &mut Cx<Self>) {
            cx.ctx_mut().push("exit Dim".to_string());
        }
        #[entry(Mute)]
        fn enter_mute(cx: &mut Cx<Self>) {
            cx.ctx_mut().push("enter Mute".to_string());
        }
        #[exit(Mute)]
        fn exit_mute(cx: &mut Cx<Self>) {
            cx.ctx_mut().push("exit Mute".to_string());
        }

        #[on(Off)]
        fn on_off(_event: &Event) -> Next<Self> {
            goto!(On { volume: 3 })
        }

        // Only the parallel state handles `Power`: no region wants it.
        #[on(On)]
        fn on_on(_on: OnFields<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Power => goto!(Off),
                _ => unhandled!(),
            }
        }

        #[on(Dim)]
        fn on_dim(event: &Event) -> Next<Self> {
            match event {
                Event::Flip => goto!(Bright),
                _ => bubble!(),
            }
        }
        #[on(Bright)]
        fn on_bright(event: &Event) -> Next<Self> {
            match event {
                Event::Flip => goto!(Dim),
                _ => bubble!(),
            }
        }
        #[on(Mute)]
        fn on_mute(event: &Event) -> Next<Self> {
            match event {
                Event::Flip => goto!(Loud),
                _ => bubble!(),
            }
        }
        #[on(Loud)]
        fn on_loud(event: &Event) -> Next<Self> {
            match event {
                Event::Flip => goto!(Mute),
                _ => bubble!(),
            }
        }
    }
}

use panel::Panel;

fn main() {
    let mut machine = Machine::<Panel>::new(Vec::new());
    machine.start().unwrap();

    // The state, then the regions in declaration order.
    machine.send(Event::Flip).unwrap();
    assert_eq!(
        std::mem::take(machine.ctx_mut()),
        ["enter On (volume 3)", "enter Dim", "enter Mute"]
    );
    assert_eq!(machine.path().to_string(), "On/light:Dim,sound:Mute");

    // Leaving exits the regions in reverse order, then the state.
    machine.send(Event::Power).unwrap();
    assert_eq!(
        std::mem::take(machine.ctx_mut()),
        ["exit Mute", "exit Dim", "exit On"]
    );
}
```

The `Flip` event in this example only moved the machine from `Off` to `On`. The regions did not see it, because they were not active yet.

## How an event reaches the regions

When an event arrives and a parallel state is active, the machine does not stop at the first handler. It asks **every region**, in declaration order:

1. Each region dispatches from its own leaf up to the top of the region, with the usual bubbling.
2. If a region's answer is a transition that stays **inside** the region, it is applied at once, before the next region looks at the event.
3. If a region's answer is a transition that leaves the parallel state, it is applied and the remaining regions are **not** evaluated.
4. The event is **consumed** when at least one region answered `stay!()`, `goto!` or `reenter!()`.
5. If no region consumed it and at least one answered `defer!()`, the event is deferred.
6. If none consumed or deferred it, the handler of the parallel state itself runs, as a normal `bubble!()` would.

This example shows each rule with a log. `Flip` is handled by both regions. `Louder` is handled by neither, so the parallel state's own handler runs. `Kill` is handled by the first region with a transition out of the parallel state, so the second region never looks at it.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Flip,
    Louder,
    Kill,
}

#[estado::machine(event = Event, ctx = Vec<String>)]
mod rig {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Rig {
        #[initial]
        Idle,
        Running(Running),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Running {
        pub volume: u8,
        #[region]
        pub light: Light,
        #[region]
        pub sound: Sound,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Light {
        #[initial]
        Dim,
        Bright,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Sound {
        #[initial]
        Mute,
        Loud,
    }

    #[handlers]
    impl Rig {
        #[on(Idle)]
        fn on_idle(_event: &Event) -> Next<Self> {
            goto!(Running { volume: 0 })
        }

        #[on(Running)]
        fn on_running(running: RunningFields<'_>, cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            cx.ctx_mut().push("Running handles".to_string());
            match event {
                Event::Louder => {
                    *running.volume += 1;
                    stay!()
                }
                _ => unhandled!(),
            }
        }

        #[on(Dim)]
        fn on_dim(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            cx.ctx_mut().push("Dim handles".to_string());
            match event {
                Event::Flip => goto!(Bright),
                // Leaves the parallel state: `sound` is skipped.
                Event::Kill => goto!(Idle),
                _ => bubble!(),
            }
        }

        #[on(Bright)]
        fn on_bright(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            cx.ctx_mut().push("Bright handles".to_string());
            match event {
                Event::Flip => goto!(Dim),
                _ => bubble!(),
            }
        }

        #[on(Mute)]
        fn on_mute(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            cx.ctx_mut().push("Mute handles".to_string());
            match event {
                Event::Flip => goto!(Loud),
                _ => bubble!(),
            }
        }

        #[on(Loud)]
        fn on_loud(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            cx.ctx_mut().push("Loud handles".to_string());
            match event {
                Event::Flip => goto!(Mute),
                _ => bubble!(),
            }
        }
    }
}

use rig::Rig;

fn main() {
    let mut machine = Machine::<Rig>::new(Vec::new());
    machine.send(Event::Flip).unwrap(); // Idle -> Running
    machine.ctx_mut().clear();

    // Both regions handle `Flip`, light first.
    machine.send(Event::Flip).unwrap();
    assert_eq!(machine.ctx(), &["Dim handles", "Mute handles"]);
    assert_eq!(machine.path().to_string(), "Running/light:Bright,sound:Loud");
    machine.ctx_mut().clear();

    // No region handles `Louder`: both bubble, then the parallel state decides.
    machine.send(Event::Louder).unwrap();
    assert_eq!(machine.ctx(), &["Bright handles", "Loud handles", "Running handles"]);
    machine.ctx_mut().clear();

    // Flip both back, then `Kill`: `light` leaves, `sound` never looks.
    machine.send(Event::Flip).unwrap();
    machine.ctx_mut().clear();
    machine.send(Event::Kill).unwrap();
    assert_eq!(machine.ctx(), &["Dim handles"]);
    assert_eq!(*machine.state(), Rig::Idle);
}
```

## Entering a region at a chosen state

The path form of `goto!` skips the regions: name a state inside a region and the macro enters it there. The other regions start in their `#[initial]`.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Start,
    StartBright,
}

#[estado::machine(event = Event)]
mod lamp {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Lamp {
        #[initial]
        Off,
        On(On),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct On {
        pub watts: u8,
        #[region]
        pub light: Light,
        #[region]
        pub timer: Timer,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Light {
        #[initial]
        Dim,
        Bright,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Timer {
        #[initial]
        Counting,
        Paused,
    }

    #[handlers]
    impl Lamp {
        #[on(Off)]
        fn on_off(event: &Event) -> Next<Self> {
            match event {
                Event::Start => goto!(On { watts: 40 }),
                // `light` starts in `Bright`; `timer` in its initial.
                Event::StartBright => goto!(On { watts: 60 } / Bright),
            }
        }

        #[on(Dim)]
        fn on_dim(_event: &Event) -> Next<Self> {
            goto!(Bright)
        }
        #[on(Bright)]
        fn on_bright(_event: &Event) -> Next<Self> {
            goto!(Dim)
        }
        #[on(Counting)]
        fn on_counting(_event: &Event) -> Next<Self> {
            goto!(Paused)
        }
        #[on(Paused)]
        fn on_paused(_event: &Event) -> Next<Self> {
            goto!(Counting)
        }
    }
}

use lamp::Lamp;

fn main() {
    let mut plain = Machine::<Lamp>::new(());
    plain.send(Event::Start).unwrap();
    assert_eq!(plain.path().to_string(), "On/light:Dim,timer:Counting");

    let mut bright = Machine::<Lamp>::new(());
    bright.send(Event::StartBright).unwrap();
    assert_eq!(bright.path().to_string(), "On/light:Bright,timer:Counting");
}
```

## Next

[Region coordination](./14-region-coordination.md) shows how regions look at each other and how they complete.
