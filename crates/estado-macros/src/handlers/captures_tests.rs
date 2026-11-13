//! The E0046 check keeps catching real captures while it follows scopes
//! (ADR 0013 amendment 15): no false negatives.

use proc_macro2::TokenStream;
use quote::quote;

use crate::expand;

/// Expands a machine whose handler `running` borrows `up` and returns `body`.
fn expand_with_body(body: TokenStream) -> String {
    expand::machine(
        quote! { event = Ev },
        quote! {
            mod m {
                #[root]
                pub enum M { #[initial] Idle, Active(Active), Failed { code: u8 } }
                #[state]
                pub struct Active { pub rounds: u8, #[substates] pub mode: Mode }
                #[state]
                pub enum Mode { #[initial] Running }
                #[handlers]
                impl M {
                    #[on(Running)]
                    fn running(up: ActiveFields<'_>, event: &Ev) -> Next<Self> {
                        #body
                    }
                }
            }
        },
    )
    .to_string()
}

fn is_rejected(body: TokenStream) -> bool {
    expand_with_body(body).contains("E0046")
}

#[test]
fn a_method_call_on_the_parameter_is_caught() {
    assert!(is_rejected(quote! {
        goto!(Failed, |old: Exiting![Active]| M::Failed { code: up.rounds.clone() })
    }));
}

#[test]
fn a_borrow_of_the_parameter_in_a_call_is_caught() {
    assert!(is_rejected(quote! {
        goto!(Failed, |old: Exiting![Active]| M::Failed { code: read(&up.rounds) })
    }));
}

#[test]
fn a_use_inside_a_nested_closure_is_caught() {
    assert!(is_rejected(quote! {
        goto!(Failed, |old: Exiting![Active]| {
            let add = |extra: u8| extra + *up.rounds;
            M::Failed { code: add(1) }
        })
    }));
}

#[test]
fn a_use_in_the_arguments_of_a_macro_is_caught() {
    assert!(is_rejected(quote! {
        goto!(Failed, |old: Exiting![Active]| {
            M::Failed { code: format!("{}", up.rounds).len() as u8 }
        })
    }));
}

#[test]
fn an_inline_format_argument_is_caught() {
    assert!(is_rejected(quote! {
        goto!(Failed, |old: Exiting![Active]| {
            M::Failed { code: format!("{up:?}").len() as u8 }
        })
    }));
}

#[test]
fn a_use_after_the_scope_of_a_shadowing_binding_ended_is_caught() {
    assert!(is_rejected(quote! {
        goto!(Failed, |old: Exiting![Active]| {
            { let up = old.rounds; let _ = up; }
            M::Failed { code: *up.rounds }
        })
    }));
}

#[test]
fn a_use_after_a_closure_that_shadows_the_name_is_caught() {
    assert!(is_rejected(quote! {
        goto!(Failed, |old: Exiting![Active]| {
            let keep = |up: u8| up;
            M::Failed { code: keep(1) + *up.rounds }
        })
    }));
}

#[test]
fn the_initialiser_of_a_let_still_sees_the_parameter() {
    assert!(is_rejected(quote! {
        goto!(Failed, |old: Exiting![Active]| {
            let up = *up.rounds;
            M::Failed { code: up }
        })
    }));
}

#[test]
fn a_binding_made_after_the_goto_does_not_hide_a_capture() {
    assert!(is_rejected(quote! {
        let result = goto!(Failed, |old: Exiting![Active]| M::Failed { code: *up.rounds });
        let up = 1;
        let _ = up;
        result
    }));
}

#[test]
fn a_struct_field_named_like_the_parameter_does_not_hide_a_capture() {
    assert!(is_rejected(quote! {
        goto!(Failed, |old: Exiting![Active]| {
            let info = Info { up: 1 };
            M::Failed { code: info.up + *up.rounds }
        })
    }));
}
