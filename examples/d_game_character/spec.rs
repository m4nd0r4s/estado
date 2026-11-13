//! The `Spec` impl: what the core needs from the generated code (spec 4.1,
//! 7.1).

use estado_core::{
    DeferOverflowPolicy, Graph, InlineEffects, Outcome, Sink, Slot, Spec, StateId, StatePath,
    TimerKey, UnhandledPolicy,
};

use super::actions::{CharacterCx, enter_root};
use super::ctx::GameCtx;
use super::dispatch::dispatch;
use super::graph::GRAPH;
use super::model::{Alive, Character, Command, DEFER_OVERFLOW_POLICY, Event, Movement, Weapon};
use super::runtime::CharacterRuntime;
use super::tasks::{firing_timer, jump_timer, reload_timer};

impl Spec for Character {
    type Event = Event;
    type Effect = Command;
    type Ctx = GameCtx;
    type Output = ();
    type Runtime = CharacterRuntime;
    type Effects = InlineEffects<Command, 4>;

    const NAME: &'static str = "Character";
    const GRAPH: &'static Graph = &GRAPH;
    const UNHANDLED: UnhandledPolicy = UnhandledPolicy::Ignore;
    const DEFER_OVERFLOW: DeferOverflowPolicy = DEFER_OVERFLOW_POLICY;
    /// `weapon` and `anim` read their sibling regions through `cx.in_state`.
    const TRACKS_ACTIVE: bool = true;

    fn initial() -> Self {
        Self::Alive(Alive::fresh())
    }

    fn state_id(&self) -> StateId {
        match self {
            Self::Alive(_) => Self::ALIVE_ID,
            Self::Dead => Self::DEAD_ID,
        }
    }

    fn path(&self) -> StatePath {
        self.active_path()
    }

    fn enter_initial<S: Sink<Command>>(&mut self, cx: &mut CharacterCx<'_, S>) {
        enter_root(self, cx);
    }

    fn microstep<S: Sink<Command>>(
        slot: &mut Slot<Self>,
        cx: &mut CharacterCx<'_, S>,
        event: &Event,
    ) -> Outcome {
        dispatch(slot, cx, event)
    }

    /// The event of a timer, built when it fires from the data of the state
    /// (spec 7.1). `None` unless the key names the active instance: right
    /// state and current epoch.
    fn timer_event(&self, runtime: &CharacterRuntime, key: TimerKey) -> Option<Event> {
        let Self::Alive(alive) = self else {
            return None;
        };
        if alive.movement == Movement::Jumping && key == jump_timer(runtime.jumping_epoch) {
            return Some(Event::Land);
        }
        match alive.weapon {
            Weapon::Firing if key == firing_timer(runtime.firing_epoch) => {
                Some(Event::CooldownElapsed)
            }
            Weapon::Reloading if key == reload_timer(runtime.reloading_epoch) => {
                Some(Event::ReloadDone)
            }
            _ => None,
        }
    }
}
