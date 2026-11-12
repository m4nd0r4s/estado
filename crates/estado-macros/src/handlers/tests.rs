//! Unit tests of the handler analysis, through the whole expansion.

use proc_macro2::TokenStream;
use quote::quote;

use crate::expand;

fn expand_machine(module: TokenStream) -> String {
    expand::machine(quote! { event = Ev, effect = Cmd }, module).to_string()
}

#[test]
fn debug_expansion() {
    let output = expand_machine(quote! {
        mod m {
            #[root]
            pub enum M { #[initial] Idle, Busy }
            #[handlers]
            impl M {
                #[on(Idle)]
                fn idle(cx: &mut Cx<Self>, event: &Ev) -> Next<Self> {
                    match event { Ev::Go => goto!(Busy), _ => bubble!() }
                }
            }
        }
    });
    println!("{output}");
}

fn timed_machine() -> String {
    expand_machine(quote! {
        mod m {
            #[root]
            pub enum M {
                #[initial]
                Idle,
                #[after(Duration::ZERO => Ev::Tick)]
                Backoff { attempt: u8 },
                Group(Group),
            }
            #[state]
            pub struct Group { pub size: u8, #[substates] sub: GroupSub }
            #[state]
            pub enum GroupSub { #[initial] First }
            #[handlers]
            impl M {}
        }
    })
}

#[test]
fn the_shared_view_of_a_timed_variant_is_hidden_from_the_docs() {
    let output = timed_machine();
    let at = output.find("struct BackoffRef").expect("the view exists");
    assert!(
        output[..at].ends_with("# [doc (hidden)] pub struct BackoffRef")
            || output[..at].trim_end().ends_with("# [doc (hidden)] pub")
    );
}

#[test]
fn helpers_generated_on_user_types_are_hidden_and_prefixed() {
    let output = timed_machine();
    assert!(
        !output.contains("fn split"),
        "a user method `split` would clash"
    );
    assert!(output.contains("fn __estado_split"));
    assert!(output.contains("# [doc (hidden)] # [allow (dead_code)] fn __estado_split"));
}
