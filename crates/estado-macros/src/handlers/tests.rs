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
