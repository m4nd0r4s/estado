//! Handlers and the `Spec` impl of `Tree`: nested `match` dispatch from the
//! leaf up (spec 5.1), then the chosen edge applies in the same frame.

use estado_core::{
    Edge, Graph, InlineEffects, Next, Outcome, Sink, Slot, Spec, StateId, StatePath,
};

use super::actions::*;
use super::edges::*;
use super::model::*;

// Handlers, as the user writes them after `#[handlers]` rewrote `goto!`.

fn on_normal<S: Sink<()>>(cx: &mut TreeCx<'_, S>, event: &Event) -> Next<Tree, NormalEdges> {
    note(cx, "handler Normal");
    match event {
        Event::ToPaused => Next::Goto(NormalEdges::ToPaused),
        Event::NormalToFast => Next::Goto(NormalEdges::ToFast),
        Event::NormalToActive => Next::Goto(NormalEdges::ToActiveLocal),
        Event::NormalToActiveExternal => Next::Goto(NormalEdges::ToActiveExternal),
        Event::NormalToItself => Next::Goto(NormalEdges::ToItself),
        _ => Next::Bubble,
    }
}

fn on_playing<S: Sink<()>>(
    playing: &mut Playing,
    cx: &mut TreeCx<'_, S>,
    event: &Event,
) -> Next<Tree, PlayingEdges> {
    note(cx, "handler Playing");
    match event {
        Event::PlayingToIdle => Next::Goto(PlayingEdges::ToIdle),
        Event::ToPaused => Next::Goto(PlayingEdges::ToPaused),
        Event::ReenterPlaying => Next::Reenter,
        Event::SetSpeed(speed) => {
            playing.speed = *speed;
            Next::Stay
        }
        _ => Next::Bubble,
    }
}

fn on_active<S: Sink<()>>(
    count: &mut u32,
    cx: &mut TreeCx<'_, S>,
    event: &Event,
) -> Next<Tree, impl Edge<Tree> + use<S>> {
    note(cx, "handler Active");
    match event {
        Event::ActiveToFast => Next::Goto(ActiveEdges::ToFast),
        Event::StayInActive => {
            *count += 1;
            Next::Stay
        }
        Event::ActiveToStopped => Next::Goto(ActiveEdges::ToStopped(|old: ActiveExiting| {
            Tree::Stopped { count: old.count }
        })),
        _ => Next::Bubble,
    }
}

fn on_paused<S: Sink<()>>(cx: &mut TreeCx<'_, S>, event: &Event) -> Next<Tree, PausedEdges> {
    note(cx, "handler Paused");
    match event {
        Event::PausedToPlayingWithHistory => Next::Goto(PausedEdges::ToPlayingWithHistory),
        _ => Next::Bubble,
    }
}

fn on_outside<S: Sink<()>>(cx: &mut TreeCx<'_, S>, event: &Event) -> Next<Tree, OutsideEdges> {
    note(cx, "handler outside");
    match event {
        Event::ToActive => Next::Goto(OutsideEdges::ToActive),
        Event::ToActiveWithHistory => Next::Goto(OutsideEdges::ToActiveWithHistory),
        _ => Next::Unhandled,
    }
}

/// What the dispatch does after a handler returned.
enum Resolution {
    Done(Outcome),
    Bubble,
    Reenter,
}

fn resolve<E: Edge<Tree>, S: Sink<()>>(
    next: Next<Tree, E>,
    slot: &mut Slot<Tree>,
    cx: &mut TreeCx<'_, S>,
) -> Resolution {
    match next {
        Next::Stay => Resolution::Done(Outcome::Stayed),
        Next::Bubble => Resolution::Bubble,
        Next::Unhandled => Resolution::Done(Outcome::Unhandled(Tree::UNHANDLED)),
        Next::Defer => Resolution::Done(Outcome::Deferred),
        Next::Reenter => Resolution::Reenter,
        Next::Goto(edge) => Resolution::Done(Outcome::Transitioned {
            changed: edge.apply(slot, cx),
        }),
        Next::__Never(_, never) => match never {},
    }
}

/// `reenter!()` from `Playing`: LCA `Active`, the state keeps its own data
/// and its substates restart from the initial (ADR 0005).
fn reenter_playing<S: Sink<()>>(slot: &mut Slot<Tree>, cx: &mut TreeCx<'_, S>) -> Outcome {
    let Tree::Active(active) = slot.live_mut() else {
        unreachable!("Playing is active")
    };
    let ActiveSub::Playing(playing) = &mut active.sub else {
        unreachable!("Playing is active")
    };
    let leaf_before = playing.sub;
    exit_playing(playing, cx);
    playing.sub = PlayingSub::Normal;
    enter_playing(playing, cx);
    // The configuration changes when the substate was not the initial (ADR 0006).
    Outcome::Transitioned {
        changed: leaf_before != PlayingSub::Normal,
    }
}

fn bubble_to_active<S: Sink<()>>(
    slot: &mut Slot<Tree>,
    cx: &mut TreeCx<'_, S>,
    event: &Event,
) -> Outcome {
    let Tree::Active(active) = slot.live_mut() else {
        unreachable!("Active is active")
    };
    let next = on_active(&mut active.count, cx, event);
    match resolve(next, slot, cx) {
        Resolution::Done(outcome) => outcome,
        // The root has no handler: bubbling from a top-level state is unhandled.
        Resolution::Bubble => Outcome::Unhandled(Tree::UNHANDLED),
        Resolution::Reenter => unreachable!("Active does not reenter"),
    }
}

fn dispatch_in_playing<S: Sink<()>>(
    slot: &mut Slot<Tree>,
    cx: &mut TreeCx<'_, S>,
    event: &Event,
) -> Outcome {
    let leaf = match slot.live_mut() {
        Tree::Active(Active {
            sub: ActiveSub::Playing(playing),
            ..
        }) => playing.sub,
        _ => unreachable!("Playing is active"),
    };
    if leaf == PlayingSub::Normal {
        let next = on_normal(cx, event);
        match resolve(next, slot, cx) {
            Resolution::Done(outcome) => return outcome,
            Resolution::Reenter => unreachable!("Normal does not reenter"),
            Resolution::Bubble => {}
        }
    }
    // `Fast` has no handler: equivalent to `bubble!()`.
    let Tree::Active(Active {
        sub: ActiveSub::Playing(playing),
        ..
    }) = slot.live_mut()
    else {
        unreachable!("Playing is active")
    };
    let next = on_playing(playing, cx, event);
    match resolve(next, slot, cx) {
        Resolution::Done(outcome) => outcome,
        Resolution::Reenter => reenter_playing(slot, cx),
        Resolution::Bubble => bubble_to_active(slot, cx, event),
    }
}

impl Spec for Tree {
    type Event = Event;
    type Effect = ();
    type Ctx = Vec<String>;
    type Output = ();
    type Runtime = TreeRuntime;
    type Effects = InlineEffects<(), 4>;

    const NAME: &'static str = "Tree";
    const GRAPH: &'static Graph = &GRAPH;

    fn initial() -> Self {
        Self::Idle
    }

    fn state_id(&self) -> StateId {
        self.active_path().leaf().expect("never empty")
    }

    fn path(&self) -> StatePath {
        self.active_path()
    }

    fn enter_initial<S: Sink<()>>(&mut self, cx: &mut TreeCx<'_, S>) {
        enter_root_state(self, cx);
    }

    fn microstep<S: Sink<()>>(
        slot: &mut Slot<Self>,
        cx: &mut TreeCx<'_, S>,
        event: &Event,
    ) -> Outcome {
        let in_playing = match slot.live_mut() {
            Tree::Idle | Tree::Stopped { .. } => {
                let next = on_outside(cx, event);
                return match resolve(next, slot, cx) {
                    Resolution::Done(outcome) => outcome,
                    Resolution::Bubble => Outcome::Unhandled(Tree::UNHANDLED),
                    Resolution::Reenter => unreachable!("outside states do not reenter"),
                };
            }
            Tree::Active(active) => matches!(active.sub, ActiveSub::Playing(_)),
        };
        if in_playing {
            return dispatch_in_playing(slot, cx, event);
        }
        let next = on_paused(cx, event);
        match resolve(next, slot, cx) {
            Resolution::Done(outcome) => outcome,
            Resolution::Reenter => unreachable!("Paused does not reenter"),
            Resolution::Bubble => bubble_to_active(slot, cx, event),
        }
    }
}
