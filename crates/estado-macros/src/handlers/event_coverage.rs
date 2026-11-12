//! Which events an `#[on]` handler reacts to, read from the `match` over its
//! event parameter. It feeds W0005, which must never warn by mistake, so every
//! shape this scan does not fully understand counts as "any event".

use proc_macro2::TokenTree;
use quote::ToTokens;
use syn::visit::{self, Visit};
use syn::{Arm, Block, Expr, ExprMatch, Ident, Pat, UnOp};

use crate::model::EventCoverage;
use crate::naming::facade_name;

/// The coverage of a handler body whose event parameter is `event` (`None`
/// when the function has no event parameter or ignores it with `_`).
///
/// The body must be the original one, before the rewrite: `bubble!()` and
/// `unhandled!()` are still macros there.
pub fn event_coverage(body: &Block, event: Option<&Ident>) -> EventCoverage {
    let Some(event) = event else {
        return EventCoverage::AnyEvent;
    };
    let mut scan = MatchScan {
        event,
        variants: Vec::new(),
        scrutinee_uses: 0,
        any_event: false,
        matches_over_event: 0,
    };
    scan.visit_block(body);
    let total_uses = count_identifier(body.to_token_stream(), event);
    // The event is used only as the scrutinee of the matches the scan read;
    // `if let`, `matches!`, a helper call or `event.kind()` leave it unknown.
    let only_matched = total_uses == scan.scrutinee_uses;
    if scan.any_event || scan.matches_over_event == 0 || !only_matched {
        return EventCoverage::AnyEvent;
    }
    scan.variants.sort();
    scan.variants.dedup();
    EventCoverage::Variants(scan.variants)
}

struct MatchScan<'event> {
    event: &'event Ident,
    variants: Vec<String>,
    /// Occurrences of the event identifier inside the scrutinees read.
    scrutinee_uses: usize,
    any_event: bool,
    matches_over_event: usize,
}

impl<'ast> Visit<'ast> for MatchScan<'_> {
    fn visit_expr_match(&mut self, expression: &'ast ExprMatch) {
        if is_the_event(&expression.expr, self.event) {
            self.matches_over_event += 1;
            self.scrutinee_uses += 1;
            for arm in &expression.arms {
                self.read_arm(arm);
            }
        }
        visit::visit_expr_match(self, expression);
    }
}

impl MatchScan<'_> {
    fn read_arm(&mut self, arm: &Arm) {
        match arm_pattern(&arm.pat) {
            ArmPattern::Variants(names) => self.variants.extend(names),
            ArmPattern::CatchAll => {
                if !passes_on(&arm.body) {
                    self.any_event = true;
                }
            }
            ArmPattern::Unknown => self.any_event = true,
        }
    }
}

/// `event`, `*event`, `&event` and parenthesised forms.
fn is_the_event(expression: &Expr, event: &Ident) -> bool {
    match expression {
        Expr::Path(path) => path.path.is_ident(event),
        Expr::Paren(inner) => is_the_event(&inner.expr, event),
        Expr::Reference(inner) => is_the_event(&inner.expr, event),
        Expr::Unary(inner) if matches!(inner.op, UnOp::Deref(_)) => {
            is_the_event(&inner.expr, event)
        }
        _ => false,
    }
}

enum ArmPattern {
    /// Names event variants (last path segments).
    Variants(Vec<String>),
    /// `_` or a binding: matches every event left.
    CatchAll,
    /// Anything else (tuples, literals, slices, ..).
    Unknown,
}

fn arm_pattern(pattern: &Pat) -> ArmPattern {
    match pattern {
        Pat::Wild(_) => ArmPattern::CatchAll,
        Pat::TupleStruct(inner) => variant_of(&inner.path),
        Pat::Struct(inner) => variant_of(&inner.path),
        Pat::Path(inner) => variant_of(&inner.path),
        Pat::Reference(inner) => arm_pattern(&inner.pat),
        Pat::Paren(inner) => arm_pattern(&inner.pat),
        Pat::Ident(inner) => match &inner.subpat {
            Some((_, subpattern)) => arm_pattern(subpattern),
            // `Timeout` after `use Ev::*` is a variant, `other` is a binding.
            None if starts_uppercase(&inner.ident) => {
                ArmPattern::Variants(vec![inner.ident.to_string()])
            }
            None => ArmPattern::CatchAll,
        },
        Pat::Or(inner) => {
            let mut names = Vec::new();
            for case in &inner.cases {
                match arm_pattern(case) {
                    ArmPattern::Variants(more) => names.extend(more),
                    ArmPattern::CatchAll => return ArmPattern::CatchAll,
                    ArmPattern::Unknown => return ArmPattern::Unknown,
                }
            }
            ArmPattern::Variants(names)
        }
        _ => ArmPattern::Unknown,
    }
}

fn variant_of(path: &syn::Path) -> ArmPattern {
    path.segments.last().map_or(ArmPattern::Unknown, |segment| {
        ArmPattern::Variants(vec![segment.ident.to_string()])
    })
}

fn starts_uppercase(ident: &Ident) -> bool {
    ident
        .to_string()
        .chars()
        .next()
        .is_some_and(char::is_uppercase)
}

/// The arm only passes the event on: `bubble!()` or `unhandled!()`, alone or
/// as the single expression of a block.
fn passes_on(body: &Expr) -> bool {
    match body {
        Expr::Macro(call) => {
            facade_name(&call.mac.path).is_some_and(|name| name == "bubble" || name == "unhandled")
        }
        Expr::Block(block) => match block.block.stmts.as_slice() {
            [syn::Stmt::Expr(inner, _)] => passes_on(inner),
            [syn::Stmt::Macro(call)] => facade_name(&call.mac.path)
                .is_some_and(|name| name == "bubble" || name == "unhandled"),
            _ => false,
        },
        Expr::Paren(inner) => passes_on(&inner.expr),
        _ => false,
    }
}

/// How many times `name` appears as an identifier, macro arguments included.
fn count_identifier(tokens: proc_macro2::TokenStream, name: &Ident) -> usize {
    tokens
        .into_iter()
        .map(|token| match token {
            TokenTree::Ident(ident) => usize::from(ident == *name),
            TokenTree::Group(group) => count_identifier(group.stream(), name),
            _ => 0,
        })
        .sum()
}
