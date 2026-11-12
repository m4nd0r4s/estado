//! AC-15: `Machine<M>` costs nothing over its parts (spec 3.5, errata 1,
//! ADR 0002).
//!
//! Each machine below is written by hand against `Spec` with no optional
//! feature: `NoRuntime` and a `Stay`-only microstep. Only the layout is under
//! test, so the states never do anything.

#![allow(dead_code)]

use core::mem::{align_of, size_of};

use estado_core::{
    Cx, Discard, Graph, Machine, NoRuntime, Outcome, Sink, Slot, Spec, StateId, StatePath,
};

/// Implements a `Spec` for `$root` whose only state behaviour is to stay.
macro_rules! implement_stay_only_spec {
    ($root:ty, $ctx:ty, $initial:expr) => {
        impl Spec for $root {
            type Event = ();
            type Effect = ();
            type Ctx = $ctx;
            type Output = ();
            type Runtime = NoRuntime<()>;
            type Effects = Discard;

            const NAME: &'static str = stringify!($root);
            const GRAPH: &'static Graph = &Graph {
                machine: stringify!($root),
                states: &[],
                edges: &[],
            };

            fn initial() -> Self {
                $initial
            }

            fn state_id(&self) -> StateId {
                StateId::new(0)
            }

            fn path(&self) -> StatePath {
                StatePath::new(Self::GRAPH)
            }

            fn enter_initial<S: Sink<()>>(&mut self, _cx: &mut Cx<'_, Self, S>) {}

            fn microstep<S: Sink<()>>(
                _slot: &mut Slot<Self>,
                _cx: &mut Cx<'_, Self, S>,
                _event: &(),
            ) -> Outcome {
                Outcome::Stayed
            }
        }
    };
}

/// Errata 1, first half: the root has spare niches, so the machine is
/// exactly as big as the plain pair of root and context.
fn assert_machine_is_the_plain_pair<M: Spec>() {
    assert_eq!(
        size_of::<Slot<M>>(),
        size_of::<M>(),
        "precondition: the root enum has three spare niche values"
    );
    assert_eq!(size_of::<Machine<M>>(), size_of::<(M, M::Ctx)>());
}

/// Errata 1, second half: without spare niches the excess over the plain
/// pair is at most one alignment unit.
fn assert_machine_excess_is_at_most_one_alignment<M: Spec>() {
    assert!(
        size_of::<Slot<M>>() > size_of::<M>(),
        "precondition: the root enum has no spare niche"
    );
    assert!(size_of::<Machine<M>>() <= size_of::<(M, M::Ctx)>() + align_of::<M>());
}

enum Traffic {
    Red,
    Yellow,
    Green,
}
implement_stay_only_spec!(Traffic, (), Traffic::Red);

enum TrafficWithBytePadding {
    Red,
    Yellow,
    Green,
}
implement_stay_only_spec!(TrafficWithBytePadding, u8, TrafficWithBytePadding::Red);

enum Toggle {
    Off,
    On,
}
implement_stay_only_spec!(Toggle, u64, Toggle::Off);

enum Job {
    Idle,
    Counting(u32),
    Failed { reason: String },
}
implement_stay_only_spec!(Job, (), Job::Idle);

enum JobWithBytePadding {
    Idle,
    Counting(u32),
    Failed { reason: String },
}
implement_stay_only_spec!(JobWithBytePadding, u8, JobWithBytePadding::Idle);

/// The media player of spec 3.2: a compound state with owned data.
enum Player {
    Idle,
    Active(Active),
    Failed { reason: String },
}
struct Active {
    track: u32,
    buffer: Vec<u8>,
    substate: ActiveSubstate,
}
enum ActiveSubstate {
    Playing { position_millis: u64 },
    Paused { position_millis: u64 },
}
implement_stay_only_spec!(Player, (), Player::Idle);

enum PlayerWithBytePadding {
    Idle,
    Active(Active),
    Failed { reason: String },
}
implement_stay_only_spec!(PlayerWithBytePadding, u8, PlayerWithBytePadding::Idle);

/// One variant and no niche: the slot needs an extra tag.
enum Only {
    Value(u32),
}
implement_stay_only_spec!(Only, (), Only::Value(0));

enum OnlyWithBytePadding {
    Value(u32),
}
implement_stay_only_spec!(OnlyWithBytePadding, u8, OnlyWithBytePadding::Value(0));

/// `Idle` takes the only niche of `Box`, so nothing is left for the slot.
enum Boxed {
    Idle,
    Data(Box<u32>),
}
implement_stay_only_spec!(Boxed, u8, Boxed::Idle);

#[test]
fn ac_15_unit_only_enum_without_context_has_no_overhead() {
    assert_machine_is_the_plain_pair::<Traffic>();
    assert_eq!(size_of::<Machine<Traffic>>(), 1);
}

#[test]
fn ac_15_unit_only_enum_pays_context_padding_only() {
    assert_machine_is_the_plain_pair::<TrafficWithBytePadding>();
    assert_machine_is_the_plain_pair::<Toggle>();
    assert_eq!(size_of::<Machine<Toggle>>(), 16);
}

#[test]
fn ac_15_enum_with_string_payload_has_no_overhead() {
    assert_machine_is_the_plain_pair::<Job>();
    assert_machine_is_the_plain_pair::<JobWithBytePadding>();
    assert_eq!(size_of::<Machine<Job>>(), size_of::<Job>());
}

#[test]
fn ac_15_nested_compound_state_has_no_overhead() {
    assert_machine_is_the_plain_pair::<Player>();
    assert_machine_is_the_plain_pair::<PlayerWithBytePadding>();
}

#[test]
fn ac_15_the_sum_formula_of_the_spec_is_not_the_requirement() {
    // Errata 1: with a `u8` context the padded pair is bigger than the sum.
    assert!(
        size_of::<(Player, u8)>() > size_of::<Player>() + size_of::<u8>(),
        "alignment padding makes the naive sum unreachable"
    );
    assert_eq!(
        size_of::<Machine<PlayerWithBytePadding>>(),
        size_of::<(Player, u8)>()
    );
}

#[test]
fn ac_15_root_without_niche_costs_at_most_one_alignment_unit() {
    assert_machine_excess_is_at_most_one_alignment::<Only>();
    assert_machine_excess_is_at_most_one_alignment::<OnlyWithBytePadding>();
    assert_machine_excess_is_at_most_one_alignment::<Boxed>();
}

#[test]
fn ac_15_machine_new_is_const_with_no_runtime_storage() {
    const MACHINE: Machine<Traffic> = Machine::new(());
    assert_eq!(size_of_val(&MACHINE), size_of::<Traffic>());
    assert_eq!(size_of::<NoRuntime<()>>(), 0);
}
