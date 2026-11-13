//! `Spec::initial_in`: the root initial value built with a `Cx`. It runs
//! inside the start, before the entries, under the poison rules of the
//! other user code.

use std::panic::{AssertUnwindSafe, catch_unwind};

use estado_core::{
    Cx, Effect, Graph, InlineEffects, Machine, NoRuntime, Outcome, Overflow, SendError, Sink, Slot,
    Spec, StateId, StatePath,
};

/// What `initial_in` does.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Emit,
    Panic,
    ReadContext,
    InState,
}

struct Boot {
    loaded: u32,
}

impl Spec for Boot {
    type Event = ();
    type Effect = &'static str;
    type Ctx = Mode;
    type Output = ();
    type Runtime = NoRuntime<()>;
    type Effects = InlineEffects<&'static str, 4>;

    const NAME: &'static str = "Boot";
    const GRAPH: &'static Graph = &Graph {
        machine: "Boot",
        states: &[],
        edges: &[],
    };
    const TRACKS_ACTIVE: bool = true;

    fn initial() -> Self {
        unreachable!("the machine starts through initial_in")
    }

    fn initial_in<S: Sink<&'static str>>(cx: &mut Cx<'_, Self, S>) -> Self {
        match *cx.ctx() {
            Mode::Emit => {
                cx.emit("initializer");
                Self { loaded: 1 }
            }
            Mode::Panic => panic!("the initializer failed"),
            Mode::ReadContext => Self { loaded: 7 },
            Mode::InState => Self {
                loaded: u32::from(cx.in_state(StateId::new(0))),
            },
        }
    }

    fn state_id(&self) -> StateId {
        StateId::new(0)
    }

    fn path(&self) -> StatePath {
        StatePath::new(Self::GRAPH)
    }

    fn enter_initial<S: Sink<&'static str>>(&mut self, cx: &mut Cx<'_, Self, S>) {
        cx.emit("entry");
    }

    fn microstep<S: Sink<&'static str>>(
        _slot: &mut Slot<Self>,
        _cx: &mut Cx<'_, Self, S>,
        _event: &(),
    ) -> Outcome {
        Outcome::Stayed
    }
}

fn user_effects(effects: InlineEffects<&'static str, 4>) -> Vec<&'static str> {
    effects
        .into_iter()
        .map(|effect| match effect {
            Effect::User(name) => name,
            other => panic!("unexpected effect {other:?}"),
        })
        .collect()
}

#[test]
fn the_initializer_runs_inside_the_start_before_the_entries() {
    let mut machine = Machine::<Boot>::new(Mode::Emit);

    let step = machine.start().unwrap();

    assert_eq!(user_effects(step.effects), ["initializer", "entry"]);
    assert_eq!(machine.state().loaded, 1);
}

#[test]
fn the_implicit_start_of_a_send_runs_the_initializer_too() {
    let mut machine = Machine::<Boot>::new(Mode::Emit);

    let step = machine.send(()).unwrap();

    assert_eq!(user_effects(step.effects), ["initializer", "entry"]);
    assert_eq!(machine.state().loaded, 1);
}

#[test]
fn the_initializer_reads_the_global_context() {
    let mut machine = Machine::<Boot>::new(Mode::ReadContext);
    machine.start().unwrap();
    assert_eq!(machine.state().loaded, 7);
}

#[test]
fn no_state_is_active_while_the_initializer_runs() {
    let mut machine = Machine::<Boot>::new(Mode::InState);
    machine.start().unwrap();
    assert_eq!(machine.state().loaded, 0);
}

#[test]
fn a_panic_in_the_initializer_poisons_the_machine() {
    let mut machine = Machine::<Boot>::new(Mode::Panic);

    let result = catch_unwind(AssertUnwindSafe(|| machine.start()));

    assert!(result.is_err());
    assert!(machine.is_poisoned());
    assert!(matches!(machine.start(), Err(SendError::Poisoned)));
}

/// Refuses every effect.
struct Full;

impl Sink<&'static str> for Full {
    fn push(&mut self, _effect: Effect<&'static str>) -> Result<(), Overflow> {
        Err(Overflow)
    }
}

#[test]
fn an_effect_the_initializer_cannot_emit_poisons_the_machine() {
    let mut machine = Machine::<Boot>::new(Mode::Emit);

    let error = machine.start_into(&mut Full).unwrap_err();

    assert_eq!(error, SendError::EffectOverflow);
    assert!(machine.is_poisoned());
}
