# Attributes

Everything the macro reads is an attribute on an item of one inline module. Only `#[machine]`, `#[root]`, `#[state]` and `#[handlers]` are real attribute macros. The others (`#[initial]`, `#[on]`, ..) are markers that `#[machine]` reads and removes; they exist only inside a `#[machine]` module.

Markers can be written with the crate name: `#[estado::state]`, `#[estado::initial]`.

| Attribute | Goes on | Summary |
| --- | --- | --- |
| [`#[machine(..)]`](#machine) | the module | declares the machine and its options |
| [`#[root]`](#root) | an enum | the top-level states |
| [`#[state]`](#state) | an enum or a struct | states below the root |
| [`#[initial]`](#initial) | a variant | the state entered first on its level |
| [`#[terminal]`](#terminal) | a variant | a final state |
| [`#[substates]`](#substates) | a field | the children of a compound state |
| [`#[region]`](#region) | a field | one region of a parallel state |
| [`#[history]`](#history) | a `#[state]` struct | remembers the last substate |
| [`#[after]`](#after) | a variant or struct | a timer |
| [`#[invoke]`](#invoke) | a variant or struct | a task |
| [`#[unhandled]`](#unhandled) | a variant or struct | the policy for events nobody handles |
| [`#[handlers]`](#handlers) | an `impl` block | holds the handler functions |
| [`#[on]`](#on), [`#[entry]`](#entry), [`#[exit]`](#exit), [`#[always]`](#always), [`#[on_done]`](#on_done) | a function | the role of a handler |

The attributes `#[choice]`, `#[on_child_done]`, `#[on_resume]`, `#[invariant]`, `#[edges]`, `#[on_error]`, `#[child]` and `#[children]` are planned but not supported yet. Each is reported as [`E0090`](../errors/E0090.md).

## machine

**Goes on** an inline module: `mod name { .. }`. The module must have a body and exactly one `#[root]` enum. It holds the whole machine; a machine cannot be split across modules or files. The module can be private.

**Syntax**: `#[machine(option, option = value, ..)]`. Write `#[estado::machine(..)]`, or `#[machine(..)]` after `use estado::prelude::*`.

**Rules**

- Each option can appear once. An unknown option, a repeated option, a bad value, or a flag given a value (`typestate = true`) is [`E0016`](../errors/E0016.md), with a "did you mean" suggestion.
- `event` is required ([`E0002`](../errors/E0002.md)).
- On anything but an inline module with one `#[root]` enum the error is [`E0001`](../errors/E0001.md).

### Options

| Option | Value | Default | Meaning |
| --- | --- | --- | --- |
| `event` | a type | required | The event type. Handlers take `&Event`. |
| `effect` | a type | `()` | The type of the commands the machine emits with `cx.emit`. |
| `ctx` | a type | `()` | The global context: `Machine::new(ctx)`, `cx.ctx()`, `cx.ctx_mut()`. |
| `unhandled` | `ignore`, `error`, `panic` | `ignore` | What happens to an event nobody handles. See [`unhandled!`](macros.md#unhandled). |
| `defer_cap` | integer | `8` | Capacity of the defer queue. |
| `defer_overflow` | `error`, `drop_oldest` | `error` | A full defer queue returns `SendError::DeferOverflow`, or drops the oldest deferred event. |
| `raise_cap` | integer | `8` | Capacity of the internal queue used by `cx.raise`. A full queue is `SendError::RaiseOverflow` and poisons the machine. |
| `fx_inline` | integer | `4` | Effects stored inline in a `Step`. More spill to the heap with `alloc`; without `alloc` it is `SendError::EffectOverflow`. |
| `max_microsteps` | integer (`u16`) | `64` | The [livelock budget](errors.md#the-livelock-budget) of one `send`. |
| `size_warn` | bytes | `128` | A variant larger than this triggers [`W0004`](../errors/W0004.md). |
| `tracks_active` | flag | off | Keeps the active-configuration snapshot that `cx.in_state` reads. See below. |
| `allow(W0001, ..)` | warning codes | none | Silences warnings. See [errors](errors.md#warnings). |
| `typestate` | flag | off | Reserved. The macro accepts it and rejects regions with it ([`E0050`](../errors/E0050.md)); it generates nothing else yet. |
| `infallible` | flag | off | Reserved. Accepted; no effect yet. |
| `version` | integer | `1` | Reserved for snapshots. Accepted; no effect yet. |

The options reach the `Spec` implementation as associated constants: `Spec::UNHANDLED`, `Spec::DEFER_OVERFLOW`, `Spec::MAX_MICROSTEPS` and `Spec::TRACKS_ACTIVE`.

The macro generates the raise and defer queues only when it sees `cx.raise(..)` or `defer!()` in the module, or when you write `raise_cap` or `defer_cap`. It generates the active-configuration snapshot only when it sees `cx.in_state(..)` or you write `tracks_active`. A machine that needs none of these stays zero-sized beyond its state.

The scan looks for method calls and path calls (`cx.raise(..)`, `Cx::raise(cx, ..)`, `Cx::<..>::in_state(..)`). A helper function outside the module is invisible to it. If a helper outside the module calls `raise` or `in_state`, switch the feature on by hand with `raise_cap = N` or `tracks_active`.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Press,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Click,
}

#[estado::machine(
    event = Event,
    effect = Command,
    ctx = u32,
    unhandled = error,
    defer_cap = 4,
    defer_overflow = drop_oldest,
    raise_cap = 2,
    fx_inline = 8,
    max_microsteps = 16,
    size_warn = 256,
    allow(W0002)
)]
mod button {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Button {
        #[initial]
        Up,
        Down,
    }

    #[handlers]
    impl Button {
        #[on(Up)]
        fn on_up(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Press => {
                    *cx.ctx_mut() += 1;
                    cx.emit(Command::Click);
                    goto!(Down)
                }
            }
        }

        #[on(Down)]
        fn on_down(event: &Event) -> Next<Self> {
            match event {
                Event::Press => unhandled!(),
            }
        }
    }
}

use button::Button;

fn main() {
    assert_eq!(<Button as Spec>::MAX_MICROSTEPS, 16);
    assert_eq!(<Button as Spec>::UNHANDLED, estado::UnhandledPolicy::Error);

    let mut machine = Machine::<Button>::new(0);
    machine.send(Event::Press).unwrap();
    assert_eq!(*machine.ctx(), 1);
    // `Down` does not handle `Press`, and the policy is `error`.
    assert_eq!(
        machine.send(Event::Press).unwrap_err(),
        SendError::Unhandled(Event::Press)
    );
}
```

## root

**Goes on** an enum inside the `#[machine]` module. Its variants are the top-level states. The enum is also the machine type: `Machine::<Root>::new(..)`, and the macro implements `Spec` for it and adds the [state id constants](context.md#state-ids).

**Syntax**: `#[root]`, no arguments.

**Rules**

- Exactly one per module ([`E0001`](../errors/E0001.md) for none or several, or for a non-enum).
- Exactly one variant is `#[initial]` ([`E0010`](../errors/E0010.md)).
- Variants can be unit, newtype, tuple or struct-like. A newtype variant whose payload is a `#[state]` struct is a composite state when the struct has a `#[substates]` or a `#[region]` field. Without one, it is an atomic state with data.
- `#[derive]`s and other attributes on the enum are kept. The macro does not require any trait.

## state

**Goes on** an enum or a struct inside the `#[machine]` module.

**Syntax**: `#[state]`, no arguments.

- A `#[state]` **enum** is a set of sibling states. It is the type of a `#[substates]` field or of a `#[region]` field.
- A `#[state]` **struct** is the data of a compound or parallel state when it has a `#[substates]` or `#[region]` field. It is the payload of a newtype variant, such as `Active(Active)`. A `#[state]` struct without those fields is plain data of an atomic state. Its fields other than `#[substates]` and `#[region]` are the shared fields that the handlers of the substates reach through [`<Struct>Fields`](handlers.md#generated-view-types).

**Rules**

- The type of a `#[substates]` or `#[region]` field must be a `#[state]` enum named by a plain path ([`E0015`](../errors/E0015.md)). The error suggests adding `#[state]` when the enum exists without it.
- A `#[state]` enum cannot contain itself, directly or through its substates ([`E0011`](../errors/E0011.md)).
- The nesting depth, regions included, is at most 16 levels ([`E0011`](../errors/E0011.md)).
- State names are unique across the whole machine, regions included ([`E0012`](../errors/E0012.md)). The id constants live on the root type next to the variants of the root enum, so only a clash with a **root-level** state name is rejected. A root state named like its own id constant, such as `IO` or a single capital letter like `A`, is rejected, and so is a state whose constant equals the name of a root variant. Use CamelCase. A substate named `IO` is fine when no root variant has the name `IO`.
- `#[state]` on anything else is `E0017`; outside a `#[machine]` module it is `E0001`.

## initial

**Goes on** one variant of each enum of states: the root, every `#[substates]` enum and every `#[region]` enum.

**Syntax**: `#[initial]` or `#[initial(with = path)]`.

| Argument | Meaning |
| --- | --- |
| none | Enter this variant when its level is entered without an explicit target. |
| `with = path` | `path` is a function `fn() -> Enum` that builds the whole value of the variant, nested substates included. |

**Rules**

- Exactly one per enum ([`E0010`](../errors/E0010.md)).
- It goes on the variant, never on the struct ([`E0017`](../errors/E0017.md)).
- A state that has fields and is entered through `#[initial]` needs `with = ..`, because no `goto!` supplies its data ([`E0020`](../errors/E0020.md)). The same applies to the initial chain of the root, which `Machine::start` builds.
- `with = path` is the only accepted argument ([`E0017`](../errors/E0017.md) for anything else).

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Play,
    Skip,
    Stop,
}

#[estado::machine(event = Event, allow(W0001, W0002))]
mod player {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Player {
        #[initial]
        Stopped,
        Active(Active),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Active {
        pub volume: u8,
        #[substates]
        pub mode: Mode,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Mode {
        // `Playing` has data, so it needs a constructor to be an initial state.
        #[initial(with = playing_first_track)]
        Playing(Track),
        Idle,
    }

    #[derive(Debug, PartialEq, Eq)]
    pub struct Track(pub u32);

    fn playing_first_track() -> Mode {
        Mode::Playing(Track(1))
    }

    #[handlers]
    impl Player {
        #[on(Stopped)]
        fn on_stopped(event: &Event) -> Next<Self> {
            match event {
                // `Active` has a field and no `with`, so the data is given here.
                // `Mode` starts in its `#[initial]`, built by `playing_first_track`.
                Event::Play => goto!(Active { volume: 5 }),
                _ => unhandled!(),
            }
        }

        #[on(Playing)]
        fn on_playing(track: &mut Track, event: &Event) -> Next<Self> {
            match event {
                Event::Skip => {
                    track.0 += 1;
                    stay!()
                }
                _ => bubble!(),
            }
        }

        #[on(Active)]
        fn on_active(event: &Event) -> Next<Self> {
            match event {
                Event::Stop => goto!(Stopped),
                _ => unhandled!(),
            }
        }
    }
}

use player::{Active, Mode, Player, Track};

fn main() {
    let mut machine = Machine::<Player>::new(());
    machine.send(Event::Play).unwrap();
    machine.send(Event::Skip).unwrap();
    assert_eq!(
        *machine.state(),
        Player::Active(Active {
            volume: 5,
            mode: Mode::Playing(Track(2)),
        })
    );
}
```

## terminal

**Goes on** a variant.

**Syntax**: `#[terminal]`, no arguments.

**Rules**

- A `#[terminal]` variant of the **root** ends the machine: `Machine::is_done()` is true after it is entered, and later events are ignored. A newtype payload is the machine's `Output`, which `take_output` moves out. See [machine](machine.md#reading-the-state).
- A `#[terminal]` variant inside a compound or parallel state completes that state. A compound state completes when its `#[substates]` enum reaches a terminal variant; a parallel state completes when every region is in a terminal variant. The completion runs the state's [`#[on_done]`](#on_done) handler, if it has one.
- A terminal state cannot transition out: a `goto!` in a function of a `#[terminal]` state is [`E0022`](../errors/E0022.md). Only the parent or an ancestor leaves it.
- A terminal state does not trigger the dead-end warning [`W0002`](../errors/W0002.md).
- It goes on the variant, not on a struct ([`E0017`](../errors/E0017.md)).

See the [`#[on_done]` example](#on_done).

## substates

**Goes on** a named field of a `#[state]` struct. The field's type is a `#[state]` enum.

**Syntax**: `#[substates]`, no arguments.

**Rules**

- A struct with a `#[substates]` field is a compound state. It has exactly one such field ([`E0013`](../errors/E0013.md) for more).
- It cannot also have `#[region]` fields ([`E0013`](../errors/E0013.md)).
- The field must be named. A tuple field is [`E0017`](../errors/E0017.md).
- The type must be a `#[state]` enum in the same module, written as a plain name ([`E0015`](../errors/E0015.md)).

The enum is the level of substates, and it needs its own `#[initial]`. A handler of a substate receives the other fields of the struct through its `*Fields` view.

## region

**Goes on** a named field of a `#[state]` struct. The field's type is a `#[state]` enum.

**Syntax**: `#[region]`, no arguments.

**Rules**

- A struct with two or more `#[region]` fields is a parallel state: all regions are active at once. Each region enum needs an `#[initial]`.
- One `#[region]` field is [`E0014`](../errors/E0014.md); use `#[substates]` instead.
- A struct cannot have both `#[substates]` and `#[region]` fields ([`E0013`](../errors/E0013.md)); wrap the regions in a substate.
- The field name is the name of the region. It is a state name like any other, so it must be unique ([`E0012`](../errors/E0012.md)), and it has an id constant such as `Panel::LIGHT_ID`.
- A transition between states of two sibling regions is [`E0021`](../errors/E0021.md). Raise an event with `cx.raise(..)` and let the other region handle it.
- A parallel state inside a region, a handler on a region itself, and a local transition whose least common ancestor is a parallel state are not supported yet ([`E0090`](../errors/E0090.md)).

For an event, the regions run in declaration order. A `goto!` that leaves the parallel state stops the later regions. If no region handles the event, the handler of the parallel state runs. `cx.in_state` in a later region already sees the transition of an earlier one.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Start,
    Sent,
    Verified,
}

#[estado::machine(event = Event, allow(W0001))]
mod sync {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Session {
        #[initial]
        Idle,
        Running(Running),
        #[terminal]
        Closed,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Running {
        #[region]
        pub upload: Upload,
        #[region]
        pub verify: Verify,
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
    pub enum Verify {
        #[initial]
        Checking,
        #[terminal]
        Verified,
    }

    #[handlers]
    impl Session {
        #[on(Idle)]
        fn on_idle(event: &Event) -> Next<Self> {
            match event {
                Event::Start => goto!(Running),
                _ => unhandled!(),
            }
        }

        #[on(Sending)]
        fn on_sending(event: &Event) -> Next<Self> {
            match event {
                Event::Sent => goto!(Sent),
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

        // Runs when every region is in a terminal state.
        #[on_done(Running)]
        fn running_done() -> Next<Self> {
            goto!(Closed)
        }
    }
}

use sync::Session;

fn main() {
    let mut machine = Machine::<Session>::new(());
    machine.send(Event::Start).unwrap();
    assert_eq!(machine.path().to_string(), "Running/upload:Sending,verify:Checking");
    assert!(machine.is_in(Session::SENDING_ID));

    machine.send(Event::Sent).unwrap();
    assert!(!machine.is_done()); // `verify` is not finished yet

    machine.send(Event::Verified).unwrap();
    assert!(machine.is_done());
    assert_eq!(*machine.state(), Session::Closed);
}
```

## history

**Goes on** the `#[state]` struct of a compound state.

**Syntax**: `#[history]` (shallow) or `#[history(deep, keep)]` (deep).

| Form | Remembers |
| --- | --- |
| `#[history]` | Which direct substate was active. |
| `#[history(deep, keep)]` | The whole value of the substate enum, nested substates and their data included. |

`deep` and `keep` must come together. `#[history(deep)]` is [`E0017`](../errors/E0017.md).

**Rules**

- A state records its history when it exits, whichever way it exits.
- Use it with [`goto!(State, history)`](macros.md#history-form). A plain `goto!` into the state starts in its `#[initial]` substate.
- `#[history]` on a variant, or on a struct that is not compound, is [`E0017`](../errors/E0017.md).
- `goto!(S, history)` on a state without history is [`E0043`](../errors/E0043.md).
- Not supported yet ([`E0090`](../errors/E0090.md)): a shallow history on a state with more than 256 substates; `goto!(S, history)` on a shallow history whose substates carry data; a deep history nested inside another deep history; a constructor `goto!` out of a state that contains a deep history.

The example is in [`goto!`](macros.md#goto): the `Ajar` state keeps a shallow history, and `Event::Come` resumes it.

## after

**Goes on** a variant, or on the `#[state]` struct of a composite state. A state can have several.

**Syntax**: `#[after(duration => event)]`.

Both parts are expressions. Each can be a plain expression or a closure over the data of the state:

| Part | Plain | Closure |
| --- | --- | --- |
| `duration`: a `core::time::Duration` | `Duration::from_secs(3)` | <code>&#124;state&#124; Duration::from_millis(100 << *state.attempt)</code> |
| `event`: a value of the machine's event type | `Event::Timeout` | <code>&#124;state: &Online&#124; Event::Expired(state.count)</code> |

The closure parameter is a shared reference to the data of the state:

| The state is | The closure receives |
| --- | --- |
| A newtype variant, or a compound or parallel state | `&T`, the payload (the `#[state]` struct for a composite) |
| A struct-like or tuple variant | `&<Variant>Ref`, with a shared reference for each field (`*state.attempt`) |
| A unit variant | `&()` |

**Rules**

- The duration is computed when the state is entered, and a `StartTimer` effect is emitted. The event is built when the timer fires, from the data the state has then.
- Leaving the state emits `CancelTimer`. Entering it again starts a new timer with a new epoch, so a timer from an earlier visit is stale. See [effects](effects.md#keys-and-epochs).
- Timers are told apart by their position: the first `#[after]` of a state has index 0.
- At most 255 timers or invokes per state ([`E0090`](../errors/E0090.md)).
- A malformed attribute (missing `=>`) is [`E0017`](../errors/E0017.md).
- The warning [`W0005`](../errors/W0005.md) reports an event that no handler matches.

## invoke

**Goes on** a variant, or on the `#[state]` struct of a composite state. A state can have several.

**Syntax**: `#[invoke(task)]`, where `task` is an expression or a closure over the data of the state (the same parameter as in [`#[after]`](#after)). Its value has the machine's `effect` type.

**Rules**

- Entering the state emits `Effect::Invoke { key, task }`. A runtime runs the task and, when it finishes, calls `Machine::complete(key, event)`.
- Leaving the state emits `Effect::Cancel(key)`. A result that arrives later is stale and ignored.
- Timers and invokes of one visit share an epoch.
- A malformed attribute is [`E0017`](../errors/E0017.md).

```rust
use core::time::Duration;

use estado::prelude::*;
use estado_test::{DurationExt, Harness, assert_state};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Dial,
    Timeout,
    Connected(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Dial(u8),
}

#[estado::machine(event = Event, effect = Command, allow(W0002))]
mod link {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Link {
        #[initial]
        Idle,
        // The timer and the invoke belong to one visit of `Connecting`.
        #[after(Duration::from_secs(3) => Event::Timeout)]
        #[invoke(|connecting| Command::Dial(*connecting.attempt))]
        Connecting {
            attempt: u8,
        },
        Online,
    }

    #[handlers]
    impl Link {
        #[on(Idle)]
        fn on_idle(event: &Event) -> Next<Self> {
            match event {
                Event::Dial => goto!(Connecting { attempt: 1 }),
                _ => unhandled!(),
            }
        }

        #[on(Connecting)]
        fn on_connecting(event: &Event) -> Next<Self> {
            match event {
                Event::Timeout => goto!(Idle),
                Event::Connected(_) => goto!(Online),
                _ => unhandled!(),
            }
        }
    }
}

use link::Link;

fn main() {
    let mut harness = Harness::new(Machine::<Link>::new(()));

    // The timer fires on the virtual clock and the machine falls back to `Idle`.
    harness.send(Event::Dial);
    assert_eq!(harness.pending_timers().len(), 1);
    assert_eq!(harness.pending_invokes().len(), 1);
    harness.advance(3.secs());
    assert_state!(harness, Link::Idle);
    harness.assert_no_pending_timers();

    // The task completes first: `Online`, and the timer is cancelled.
    harness.send(Event::Dial);
    harness
        .complete_invoke(|task| matches!(task, Command::Dial(1)), Event::Connected(9));
    assert_state!(harness, Link::Online);
    harness.assert_no_pending_timers();
}
```

## unhandled

**Goes on** a variant, or on the `#[state]` struct of a composite state.

**Syntax**: `#[unhandled(ignore)]`, `#[unhandled(error)]` or `#[unhandled(panic)]`.

It overrides the machine's `unhandled` option for events that reach the state through `unhandled!()`, or that bubble out of it unhandled. The nearest attribute, the state's own or an ancestor's, applies. The state's own attribute applies at once, also when the event leaves it through `bubble!()`: the parent's handler does not run. See [`unhandled!`](macros.md#unhandled) for what each policy does.

An unknown policy is [`E0017`](../errors/E0017.md), with a suggestion.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Edit,
    Noise,
}

#[estado::machine(event = Event, allow(W0001, W0002))]
mod doc {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Doc {
        #[initial]
        Idle,
        // Inside `Editing`, an unhandled event is an error.
        Editing(Editing),
    }

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
    }

    #[handlers]
    impl Doc {
        #[on(Idle)]
        fn on_idle(event: &Event) -> Next<Self> {
            match event {
                Event::Edit => goto!(Editing { dirty: false }),
                _ => unhandled!(), // the machine's policy: ignore
            }
        }

        #[on(Typing)]
        fn on_typing(event: &Event) -> Next<Self> {
            match event {
                _ => unhandled!(), // the policy of `Editing`: error
            }
        }
    }
}

use doc::Doc;

fn main() {
    let mut machine = Machine::<Doc>::new(());
    assert!(!machine.send(Event::Noise).unwrap().report.handled);

    machine.send(Event::Edit).unwrap();
    assert_eq!(
        machine.send(Event::Noise).unwrap_err(),
        SendError::Unhandled(Event::Noise)
    );
}
```

## handlers

**Goes on** an inherent `impl` block of the root type: `impl Root { .. }`. A module can have several.

**Syntax**: `#[handlers]`, no arguments.

**Rules**

- `impl Trait for Root`, or an `impl` of another type, is [`E0017`](../errors/E0017.md): the functions take `Self` as the machine.
- A role attribute on a constant, a type alias or a macro call inside the block is [`E0017`](../errors/E0017.md).
- Functions without a role attribute are plain helpers.

The role attributes below apply to functions in this block. Their parameters and return types are in [handlers](handlers.md).

## on

**Syntax**: `#[on(StateName)]`. The function runs when an event reaches the state: the state is the active leaf, or the event bubbled up from a substate. It takes the event and returns `Next<Self>`.

## entry

**Syntax**: `#[entry(StateName)]`. Runs when the state is entered, outermost state first. No event; returns nothing; cannot transition ([`E0042`](../errors/E0042.md)).

## exit

**Syntax**: `#[exit(StateName)]`. Runs when the state is exited, deepest state first. No event; returns nothing; cannot transition ([`E0042`](../errors/E0042.md)).

## always

**Syntax**: `#[always(StateName)]`. Runs as its own microstep right after the state is entered, before internal events and before deferred events are redelivered. It runs once per entry, not on every later event. It reacts to the state, not to an event, so it takes no event and only `stay!()` and `goto!` are allowed ([`E0045`](../errors/E0045.md)). It counts toward the [livelock budget](errors.md#the-livelock-budget).

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Measure(u32),
}

#[estado::machine(event = Event, allow(W0002))]
mod gauge {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Gauge {
        #[initial]
        Waiting,
        Checking {
            value: u32,
        },
        Low,
        High,
    }

    #[handlers]
    impl Gauge {
        #[on(Waiting)]
        fn on_waiting(event: &Event) -> Next<Self> {
            match event {
                Event::Measure(value) => goto!(Checking { value: *value }),
            }
        }

        // Decides right after `Checking` is entered; the event never sees it.
        #[always(Checking)]
        fn route(checking: CheckingMut<'_>) -> Next<Self> {
            if *checking.value > 10 {
                goto!(High)
            } else {
                goto!(Low)
            }
        }
    }
}

use gauge::Gauge;

fn main() {
    let mut machine = Machine::<Gauge>::new(());
    machine.send(Event::Measure(42)).unwrap();
    assert_eq!(*machine.state(), Gauge::High);
}
```

## on_done

**Syntax**: `#[on_done(StateName)]`. `StateName` must be a compound or parallel state ([`E0044`](../errors/E0044.md) otherwise). It runs when the state completes: when its substate enum reaches a `#[terminal]` variant, or when every region is in a terminal variant. It runs once per completion, as its own microstep, after the entries of the terminal state. It can take `&Output`, the payload of the terminal substate. Only `stay!()` and `goto!` are allowed ([`E0045`](../errors/E0045.md)).

The [handlers page](handlers.md) shows a compound state with `&Output`, and the [`#[region]` example](#region) shows a parallel state.
