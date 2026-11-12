//! Prints and asserts `size_of` for every fixture x ctx x option.
//! Run with `cargo test --test sizes -- --nocapture` to see the table.

use estado_spike_poison::fixtures_flat::*;
use estado_spike_poison::fixtures_full256::Full256;
use estado_spike_poison::fixtures_marked::{Marked, MarkedPlain};
use estado_spike_poison::fixtures_player::Player;
use estado_spike_poison::flag_machine::FlagMachine;
use estado_spike_poison::mirror_cast_machine::MirrorCastMachine;
use estado_spike_poison::mirror_machine::MirrorMachine;
use estado_spike_poison::option_machine::OptionMachine;
use estado_spike_poison::slot_machine::{Slot, SlotMachine};
use estado_spike_poison::uninit_machine::UninitMachine;
use estado_spike_poison::{Mirrored, Spec};
use std::mem::size_of;

struct Row {
    plain_enum: usize,
    sum: usize,
    tuple: usize,
    option: usize,
    slot: usize,
    flag: usize,
    uninit: usize,
    mirror: Option<usize>,
}

fn measure<M: Spec, C>() -> Row {
    Row {
        plain_enum: size_of::<M>(),
        sum: size_of::<M>() + size_of::<C>(),
        tuple: size_of::<(M, C)>(),
        option: size_of::<OptionMachine<M, C>>(),
        slot: size_of::<SlotMachine<M, C>>(),
        flag: size_of::<FlagMachine<M, C>>(),
        uninit: size_of::<UninitMachine<M, C>>(),
        mirror: None,
    }
}

fn measure_mirrored<M: Mirrored, C>() -> Row {
    let mut row = measure::<M, C>();
    row.mirror = Some(size_of::<MirrorMachine<M, C>>());
    row
}

fn report<M>(fixture: &str, ctx: &str, row: &Row, has_niche: bool) {
    let mirror = row.mirror.map_or("-".to_string(), |size| size.to_string());
    println!(
        "| {fixture:<8} | {ctx:<4} | {:>3} | {:>3} | {:>5} | {:>6} | {:>4} | {:>4} | {:>6} | {:>6} | {:>3}/{:<3} |",
        row.plain_enum,
        row.sum,
        row.tuple,
        row.option,
        row.slot,
        row.flag,
        row.uninit,
        mirror,
        size_of::<Option<M>>(),
        size_of::<Slot<M>>(),
    );
    // Option never costs more than the plain struct when a niche exists.
    if has_niche {
        assert_eq!(
            row.option, row.tuple,
            "{fixture}/{ctx}: Option<M> must be free"
        );
        assert_eq!(row.slot, row.tuple, "{fixture}/{ctx}: Slot<M> must be free");
        if let Some(mirror) = row.mirror {
            assert_eq!(mirror, row.tuple, "{fixture}/{ctx}: mirror must be free");
        }
    } else {
        assert!(row.option >= row.tuple, "{fixture}/{ctx}");
    }
    assert_eq!(size_of::<Option<M>>() == row.plain_enum, has_niche);
}

macro_rules! rows {
    ($fixture:ty, $name:literal, niche = $niche:expr, $measure:ident) => {{
        report::<$fixture>($name, "()", &$measure::<$fixture, ()>(), $niche);
        report::<$fixture>($name, "u8", &$measure::<$fixture, u8>(), $niche);
        report::<$fixture>($name, "u32", &$measure::<$fixture, u32>(), $niche);
        report::<$fixture>($name, "u64", &$measure::<$fixture, u64>(), $niche);
    }};
}

#[test]
fn size_table() {
    println!(
        "| fixture  | ctx  |   M | M+C | (M,C) | Option | slot | flag | uninit | mirror | Opt<M>/Slot<M> |"
    );
    rows!(Unit, "Unit", niche = true, measure_mirrored);
    rows!(Boxed, "Boxed", niche = false, measure_mirrored);
    rows!(Strs, "Strs", niche = true, measure);
    rows!(Owned, "Owned", niche = true, measure_mirrored);
    rows!(Pair, "Pair", niche = true, measure);
    rows!(Only, "Only", niche = false, measure_mirrored);
    rows!(Full256, "Full256", niche = false, measure);
    rows!(Player, "Player", niche = true, measure_mirrored);
}

#[test]
fn ac15_literal_holds_with_zst_ctx_and_a_niche() {
    assert_eq!(size_of::<OptionMachine<Player, ()>>(), size_of::<Player>());
    assert_eq!(size_of::<OptionMachine<Unit, ()>>(), size_of::<Unit>());
    assert_eq!(size_of::<OptionMachine<Owned, ()>>(), size_of::<Owned>());
    assert_eq!(size_of::<OptionMachine<Strs, ()>>(), size_of::<Strs>());
}

#[test]
fn ac15_literal_fails_on_alignment_padding_even_without_any_poison_support() {
    // A plain `struct { state: M, ctx: C }` already breaks `size(M) + size(C)`.
    assert_ne!(
        size_of::<(Unit, u64)>(),
        size_of::<Unit>() + size_of::<u64>()
    );
    assert_ne!(
        size_of::<(Pair, u32)>(),
        size_of::<Pair>() + size_of::<u32>()
    );
}

#[test]
fn no_niche_cases_cost_one_alignment_unit() {
    assert!(size_of::<OptionMachine<Only, ()>>() > size_of::<Only>());
    assert!(size_of::<OptionMachine<Full256, ()>>() > size_of::<Full256>());
    assert!(size_of::<OptionMachine<Boxed, ()>>() > size_of::<Boxed>());
}

#[test]
fn niche_budget_is_consumed_by_the_user_variants() {
    // `Idle` takes the only niche of `Box`: nothing is left for poisoning.
    assert_eq!(size_of::<Boxed>(), 8);
    assert_eq!(size_of::<Option<Boxed>>(), 16);
    // Three states (unstarted, live, poisoned) need two niche values.
    assert_eq!(size_of::<Slot<Player>>(), size_of::<Player>());
    assert_eq!(size_of::<Slot<Unit>>(), size_of::<Unit>());
}

#[test]
fn repr_u8_mirror_pessimises_the_user_enum() {
    println!(
        "Marked (repr(u8)) = {}, MarkedPlain = {}",
        size_of::<Marked>(),
        size_of::<MarkedPlain>()
    );
    println!(
        "MirrorCastMachine<Marked, ()> = {}",
        size_of::<MirrorCastMachine<Marked, ()>>()
    );
    assert!(size_of::<Marked>() >= size_of::<MarkedPlain>());
}

#[test]
fn slot_needs_two_niche_values_option_needs_one() {
    #[allow(dead_code)]
    enum SingleBox {
        Data(Box<u32>),
    }
    assert_eq!(size_of::<SingleBox>(), 8);
    assert_eq!(size_of::<Option<SingleBox>>(), 8);
    assert_eq!(size_of::<Slot<SingleBox>>(), 16);
}
