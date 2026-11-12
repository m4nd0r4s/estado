//! The harness on a small hand-written machine with timers and an invoke.
//!
//! `Lamp`: `Off` (`#[after(2s => Tick)]`), `On` (`#[after(1s => Tick)]` and
//! `#[after(1s => Blink)]`, same deadline), `Loading { id }`
//! (`#[invoke(|s| Command::Fetch(s.id))]`).

use core::time::Duration;

use estado_core::{
    ChildPath, Cx, Effect, Graph, InlineEffects, InvokeKey, Machine, NoRuntime, Outcome, Runtime,
    Sink, Slot, Spec, StateId, StateInfo, StateKind, StatePath, TimerKey,
};
use estado_test::{DurationExt, Harness, assert_state};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Lamp {
    Off,
    On,
    Loading { id: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Event {
    Tick,
    Blink,
    Load(u32),
    Loaded(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Command {
    Light(bool),
    Blinked,
    Fetch(u32),
    Got(u32),
}

const OFF: StateId = StateId::new(0);
const ON: StateId = StateId::new(1);
const LOADING: StateId = StateId::new(2);

static STATES: [StateInfo; 3] = [
    StateInfo::new(OFF, "Off", None, StateKind::Atomic, 0).with_tasks(1, 0),
    StateInfo::new(ON, "On", None, StateKind::Atomic, 0).with_tasks(2, 0),
    StateInfo::new(LOADING, "Loading", None, StateKind::Atomic, 0).with_tasks(0, 1),
];
static GRAPH: Graph = Graph {
    machine: "Lamp",
    states: &STATES,
    edges: &[],
};

struct LampRuntime {
    queues: NoRuntime<Event>,
    epoch: u32,
}

impl Runtime for LampRuntime {
    type Event = Event;
    type Queues = NoRuntime<Event>;
    const NEW: Self = Self {
        queues: <NoRuntime<Event> as Runtime>::NEW,
        epoch: 0,
    };

    fn queues(&mut self) -> &mut Self::Queues {
        &mut self.queues
    }
}

impl Lamp {
    fn id(&self) -> StateId {
        match self {
            Self::Off => OFF,
            Self::On => ON,
            Self::Loading { .. } => LOADING,
        }
    }
}

fn timer(state: StateId, epoch: u32, index: u8) -> TimerKey {
    TimerKey {
        state,
        epoch,
        index,
        child: ChildPath::ROOT,
    }
}

fn invoke(epoch: u32) -> InvokeKey {
    InvokeKey {
        state: LOADING,
        epoch,
        index: 0,
        child: ChildPath::ROOT,
    }
}

/// One epoch counter is enough here: only one state is active at a time.
fn entry<S: Sink<Command>>(state: &Lamp, cx: &mut Cx<'_, Lamp, S>) {
    cx.runtime_mut().epoch += 1;
    let epoch = cx.runtime_mut().epoch;
    match state {
        Lamp::Off => {
            cx.emit(Command::Light(false));
            cx.effect(Effect::StartTimer {
                key: timer(OFF, epoch, 0),
                after: Duration::from_secs(2),
            });
        }
        Lamp::On => {
            cx.emit(Command::Light(true));
            for index in 0..2 {
                cx.effect(Effect::StartTimer {
                    key: timer(ON, epoch, index),
                    after: Duration::from_secs(1),
                });
            }
        }
        Lamp::Loading { id } => cx.effect(Effect::Invoke {
            key: invoke(epoch),
            task: Command::Fetch(*id),
        }),
    }
}

fn exit<S: Sink<Command>>(state: &Lamp, cx: &mut Cx<'_, Lamp, S>) {
    let epoch = cx.runtime_mut().epoch;
    match state {
        Lamp::Off => cx.effect(Effect::CancelTimer(timer(OFF, epoch, 0))),
        Lamp::On => {
            for index in 0..2 {
                cx.effect(Effect::CancelTimer(timer(ON, epoch, index)));
            }
        }
        Lamp::Loading { .. } => cx.effect(Effect::Cancel(invoke(epoch))),
    }
}

impl Spec for Lamp {
    type Event = Event;
    type Effect = Command;
    type Ctx = ();
    type Output = ();
    type Runtime = LampRuntime;
    type Effects = InlineEffects<Command, 4>;

    const NAME: &'static str = "Lamp";
    const GRAPH: &'static Graph = &GRAPH;

    fn initial() -> Self {
        Self::Off
    }

    fn state_id(&self) -> StateId {
        self.id()
    }

    fn path(&self) -> StatePath {
        let mut path = StatePath::new(&GRAPH);
        path.push(self.id());
        path
    }

    fn enter_initial<S: Sink<Command>>(&mut self, cx: &mut Cx<'_, Self, S>) {
        entry(self, cx);
    }

    fn microstep<S: Sink<Command>>(
        slot: &mut Slot<Self>,
        cx: &mut Cx<'_, Self, S>,
        event: &Event,
    ) -> Outcome {
        let target = match (slot.live_mut(), event) {
            (Lamp::Off, Event::Tick) => Lamp::On,
            (Lamp::On, Event::Tick) => Lamp::Off,
            (Lamp::On, Event::Blink) => {
                cx.emit(Command::Blinked);
                return Outcome::Stayed;
            }
            (Lamp::Off, Event::Load(id)) => Lamp::Loading { id: *id },
            (Lamp::Loading { .. }, Event::Loaded(value)) => {
                cx.emit(Command::Got(*value));
                Lamp::Off
            }
            _ => return Outcome::Unhandled(Self::UNHANDLED),
        };
        exit(slot.live_mut(), cx);
        slot.put(target);
        entry(slot.live_mut(), cx);
        Outcome::Transitioned { changed: true }
    }

    fn timer_event(&self, runtime: &LampRuntime, key: TimerKey) -> Option<Event> {
        if key.state != self.id() || key.epoch != runtime.epoch {
            return None;
        }
        match (self, key.index) {
            (Lamp::Off | Lamp::On, 0) => Some(Event::Tick),
            (Lamp::On, 1) => Some(Event::Blink),
            _ => None,
        }
    }

    fn accepts_invoke(&self, runtime: &LampRuntime, key: InvokeKey) -> bool {
        matches!(self, Lamp::Loading { .. }) && key == invoke(runtime.epoch)
    }
}

fn harness() -> Harness<Lamp> {
    let mut harness = Harness::new(Machine::<Lamp>::new(()));
    harness
        .start()
        .expect_effect(|command| *command == Command::Light(false));
    harness
}

#[test]
fn the_clock_starts_at_zero_and_advance_fires_due_timers() {
    let mut harness = harness();
    assert_eq!(harness.now(), Duration::ZERO);
    harness.advance(1.secs()).expect_no_effect();
    assert_state!(harness, Lamp::Off);
    harness
        .advance(1.secs())
        .expect_effect(|command| *command == Command::Light(true));
    assert_state!(harness, Lamp::On);
    assert_eq!(harness.now(), 2.secs());
}

#[test]
fn timers_with_the_same_deadline_fire_in_scheduling_order() {
    let mut harness = harness();
    harness.advance(2.secs());
    // On schedules Tick (index 0) then Blink (index 1), both at +1 s: Tick
    // fires first and leaves On, so Blink is cancelled and never fires.
    let view = harness.advance(1.secs());
    assert_eq!(view.effects, [Command::Light(false)]);
    assert_state!(harness, Lamp::Off);
}

#[test]
fn one_advance_fires_a_chain_of_timers_in_deadline_order() {
    let mut harness = harness();
    let view = harness.advance(5.secs());
    assert_eq!(
        view.effects,
        [
            Command::Light(true),
            Command::Light(false),
            Command::Light(true)
        ]
    );
    assert_eq!(harness.now(), 5.secs());
    assert_state!(harness, Lamp::On);
}

#[test]
fn exits_cancel_pending_timers() {
    let mut harness = harness();
    assert_eq!(harness.pending_timers().len(), 1);
    harness.send(Event::Load(7));
    harness.assert_no_pending_timers();
    assert_eq!(harness.pending_invokes().len(), 1);
}

#[test]
fn complete_invoke_resolves_a_pending_task() {
    let mut harness = harness();
    harness.send(Event::Load(7));
    harness
        .complete_invoke(|task| *task == Command::Fetch(7), Event::Loaded(70))
        .expect_effect(|command| *command == Command::Got(70));
    assert_state!(harness, Lamp::Off);
    assert!(harness.pending_invokes().is_empty());
}

#[test]
fn a_late_result_is_stale() {
    let mut harness = harness();
    harness.send(Event::Load(7));
    let key = harness.pending_invokes()[0].key;
    harness.complete_invoke(|_| true, Event::Loaded(1));
    let view = harness.complete_key(key, Event::Loaded(2));
    assert_eq!(view.report.ignored, Some(estado_core::IgnoreReason::Stale));
    view.expect_no_effect();
}

#[test]
#[should_panic(expected = "no emitted effect matches")]
fn expect_effect_reports_what_was_emitted() {
    let mut harness = harness();
    harness
        .send(Event::Load(1))
        .expect_effect(|command| *command == Command::Blinked);
}

#[test]
#[should_panic(expected = "does not match")]
fn assert_state_shows_the_active_path() {
    let harness = harness();
    assert_state!(harness, Lamp::On);
}
