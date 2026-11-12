//! The reading of handler bodies behind W0005 (`event_coverage`).

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Block, Ident};

use super::event_coverage::event_coverage;
use crate::model::EventCoverage;

fn coverage_of(body: TokenStream) -> EventCoverage {
    let block: Block = syn::parse2(quote! { { #body } }).expect("a block");
    let event = Ident::new("event", proc_macro2::Span::call_site());
    event_coverage(&block, Some(&event))
}

fn variants(names: &[&str]) -> EventCoverage {
    EventCoverage::Variants(names.iter().map(|name| (*name).to_owned()).collect())
}

#[test]
fn match_arms_list_the_variants_they_name() {
    let coverage = coverage_of(quote! {
        match event {
            Ev::Retry(_) => goto!(Idle),
            Ev::Stop | Ev::Quit { .. } => stay!(),
            _ => bubble!(),
        }
    });
    assert_eq!(coverage, variants(&["Quit", "Retry", "Stop"]));
}

#[test]
fn dereferenced_and_borrowed_scrutinees_are_read() {
    let coverage = coverage_of(quote! {
        match *event { Ev::Retry => stay!(), _ => unhandled!() }
    });
    assert_eq!(coverage, variants(&["Retry"]));
}

#[test]
fn a_catch_all_that_transitions_handles_any_event() {
    let coverage = coverage_of(quote! {
        match event { Ev::Retry => stay!(), _ => goto!(Idle) }
    });
    assert_eq!(coverage, EventCoverage::AnyEvent);
}

#[test]
fn a_binding_arm_that_defers_handles_any_event() {
    let coverage = coverage_of(quote! {
        match event { Ev::Retry => stay!(), other => defer!() }
    });
    assert_eq!(coverage, EventCoverage::AnyEvent);
}

#[test]
fn a_body_without_a_match_handles_any_event() {
    assert_eq!(coverage_of(quote! { goto!(Idle) }), EventCoverage::AnyEvent);
}

#[test]
fn other_uses_of_the_event_make_it_unknown() {
    let with_if_let = quote! {
        if let Ev::Retry = event { return goto!(Idle); }
        match event { Ev::Stop => stay!(), _ => bubble!() }
    };
    assert_eq!(coverage_of(with_if_let), EventCoverage::AnyEvent);
    let with_matches = quote! {
        if matches!(event, Ev::Retry) { return goto!(Idle); }
        match event { Ev::Stop => stay!(), _ => bubble!() }
    };
    assert_eq!(coverage_of(with_matches), EventCoverage::AnyEvent);
    let delegated = quote! { match event.kind() { Kind::Stop => stay!(), _ => bubble!() } };
    assert_eq!(coverage_of(delegated), EventCoverage::AnyEvent);
}

#[test]
fn tuple_and_literal_patterns_are_unknown() {
    let coverage = coverage_of(quote! {
        match event { (a, Ev::Retry) => stay!(), _ => bubble!() }
    });
    assert_eq!(coverage, EventCoverage::AnyEvent);
}

#[test]
fn an_ignored_event_handles_any_event() {
    let block: Block = syn::parse2(quote! { { goto!(Idle) } }).expect("a block");
    assert_eq!(event_coverage(&block, None), EventCoverage::AnyEvent);
}
