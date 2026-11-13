# Region coordination

Regions run side by side. This page shows how they look at each other, how an unhandled event behaves inside a region and how a parallel state completes.

## Asking about another region with `in_state`

Regions cannot transition into each other: a `goto!` between sibling regions is [E0021](../errors/E0021.md). They can still **look** at each other with `cx.in_state(Self::STATE_ID)`, which says whether a state is active in any region. Raise an event with `cx.raise` when one region needs to push another (see [Defer and raise](./18-defer-and-raise.md)).

Because regions run one after the other, the configuration that `in_state` shows is **up to date**. When `light` has just moved to `Bright` in this very event, `sound` already sees `Bright` active.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Start,
    Toggle,
}

#[estado::machine(event = Event, ctx = Vec<String>)]
mod stage {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Stage {
        #[initial]
        Closed,
        Open(Open),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Open {
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
        Beep,
    }

    #[handlers]
    impl Stage {
        #[on(Closed)]
        fn on_closed(_event: &Event) -> Next<Self> {
            goto!(Open)
        }

        #[on(Dim)]
        fn on_dim(_event: &Event) -> Next<Self> {
            goto!(Bright)
        }

        #[on(Bright)]
        fn on_bright(_event: &Event) -> Next<Self> {
            goto!(Dim)
        }

        // The sound region reacts only while the light is bright.
        #[on(Mute)]
        fn on_mute(cx: &mut Cx<Self>, _event: &Event) -> Next<Self> {
            let bright = cx.in_state(Self::BRIGHT_ID);
            cx.ctx_mut().push(format!("Mute sees bright = {bright}"));
            if bright { goto!(Beep) } else { stay!() }
        }

        #[on(Beep)]
        fn on_beep(_event: &Event) -> Next<Self> {
            goto!(Mute)
        }
    }
}

use stage::Stage;

fn main() {
    let mut machine = Machine::<Stage>::new(Vec::new());
    machine.send(Event::Start).unwrap();

    // `light` goes Dim -> Bright first. `sound` runs next and already sees it.
    machine.send(Event::Toggle).unwrap();
    assert_eq!(machine.ctx(), &["Mute sees bright = true"]);
    assert_eq!(machine.path().to_string(), "Open/light:Bright,sound:Beep");
}
```

`Self::BRIGHT_ID` is the id constant of the state `Bright`. The macro turns on the active-configuration snapshot when it sees a `cx.in_state(..)` call in the module. A call that lives in a helper outside the module is invisible to it, so add `tracks_active` to the machine attribute. See [Warnings and options](./22-warnings-and-options.md).

`machine.is_in(Stage::BRIGHT_ID)` asks the same question from outside the machine.

## `#[unhandled]` inside a region

A state in a region can carry `#[unhandled(policy)]`. An event that leaves that state unhandled then stops there: the other regions still look at it, but the handler of the parallel state does not run. The policy applies only if no sibling region handles or defers the event. See [Unhandled events](./07-unhandled-events.md).

## Completion of a parallel state

A parallel state is **complete** when every region is in a `#[terminal]` state. `#[on_done(Parallel)]` then runs, once per completion. It receives the `*Fields` of the parallel state and no payload. A region with no terminal state never completes.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Start,
    Uploaded,
    Verified,
}

#[estado::machine(event = Event, ctx = Vec<String>)]
mod release {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Release {
        #[initial]
        Waiting,
        Shipping(Shipping),
        Shipped { name: String },
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Shipping {
        pub name: String,
        #[region]
        pub upload: Upload,
        #[region]
        pub check: Check,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Upload {
        #[initial]
        Sending,
        #[terminal]
        Sent,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Check {
        #[initial]
        Checking,
        #[terminal]
        Verified,
    }

    #[handlers]
    impl Release {
        #[on(Waiting)]
        fn on_waiting(_event: &Event) -> Next<Self> {
            goto!(Shipping { name: "v1".to_string() })
        }

        #[on(Sending)]
        fn on_sending(event: &Event) -> Next<Self> {
            match event {
                Event::Uploaded => goto!(Sent),
                _ => bubble!(),
            }
        }

        #[on(Checking)]
        fn on_checking(event: &Event) -> Next<Self> {
            match event {
                Event::Verified => goto!(Verified),
                _ => bubble!(),
            }
        }

        // Runs once, when both regions are in a terminal state.
        #[on_done(Shipping)]
        fn shipping_done(shipping: ShippingFields<'_>, cx: &mut Cx<Self>) -> Next<Self> {
            cx.ctx_mut().push(format!("{} shipped", shipping.name));
            let name = shipping.name.clone();
            goto!(Shipped { name })
        }

        #[on(Shipped)]
        fn on_shipped(_shipped: ShippedMut<'_>, _event: &Event) -> Next<Self> {
            goto!(Waiting)
        }
    }
}

use release::Release;

fn main() {
    let mut machine = Machine::<Release>::new(Vec::new());
    machine.send(Event::Start).unwrap();

    machine.send(Event::Uploaded).unwrap();
    assert!(machine.ctx().is_empty()); // `check` is not done yet
    assert_eq!(machine.path().to_string(), "Shipping/upload:Sent,check:Checking");

    machine.send(Event::Verified).unwrap();
    assert_eq!(machine.ctx(), &["v1 shipped"]);
    assert_eq!(*machine.state(), Release::Shipped { name: "v1".to_string() });
}
```

## `reenter!()` on a parallel state

`reenter!()` in the handler of a parallel state exits every region and the state, then enters them again. The shared fields keep their values and every region restarts in its initial state. Only `reenter!()` keeps the shared fields. A `goto!` to the parallel state itself, such as `goto!(Run { n: 9 })`, replaces the shared fields with the data you give, and the regions restart in their initial states.

## Limits

Some shapes of regions are not generated yet and report [E0090](../errors/E0090.md): a parallel state inside a region of another parallel state, a handler on a region itself, and a local `goto!` whose common ancestor is a parallel state. Use `goto!(.., external)` or a transition inside one region instead.

## Next

[Timers](./15-timers.md) lets a state schedule events.
