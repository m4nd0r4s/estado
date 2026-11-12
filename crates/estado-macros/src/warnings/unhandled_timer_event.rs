//! W0005: an `#[after(.. => event)]` whose event no handler handles.
//!
//! Best effort (spec 6.7): the macro only sees syntax, so it warns only when
//! it is sure. The event expression must name an enum variant (a path of at
//! least two segments with a CamelCase last one, called, built as a struct, or
//! returned by a closure). Every `#[on]` handler that can see the event (see
//! [`states_in_play`]) must have a readable `match` over its event parameter
//! (see `EventCoverage`) that neither names the variant nor has a catch-all
//! arm other than `bubble!()` / `unhandled!()`. If any handler is unclear,
//! nothing is reported.

use syn::Expr;

use super::code::WarningCode;
use super::emit::Warning;
use super::scope::states_in_play;
use crate::model::{EventCoverage, HandlerRole, MachineModel};

/// One warning per timer whose event variant nothing handles.
pub fn unhandled_timer_events(model: &MachineModel) -> Vec<Warning> {
    let tree = &model.tree;
    let mut warnings = Vec::new();
    for node in tree.nodes.iter().skip(1) {
        for (index, timer) in node.timers.iter().enumerate() {
            let Some(event) = event_variant(&timer.event) else {
                continue;
            };
            let in_play = states_in_play(tree, node.id);
            let handled = model
                .analysed_handlers
                .iter()
                .filter(|handler| handler.role == HandlerRole::On)
                .filter(|handler| in_play.contains(&handler.state))
                .any(|handler| match &handler.event_coverage {
                    EventCoverage::AnyEvent => true,
                    EventCoverage::Variants(names) => names.contains(&event.name),
                });
            if handled {
                continue;
            }
            let state = &node.name;
            warnings.push(Warning {
                code: WarningCode::UnhandledTimerEvent,
                span: event.span,
                subject: format!("{}_{index}", crate::naming::plain_name(state)),
                message: format!(
                    "`#[after]` of `{state}` fires `{}`, but no handler of `{state}`, its \
                     substates or its ancestors matches it",
                    event.path
                ),
                condition: None,
            });
        }
    }
    warnings
}

/// The event variant an `#[after]` expression names.
struct EventVariant {
    /// Last path segment: `Timeout` for `Ev::Timeout`.
    name: String,
    /// The whole path as written, for the message.
    path: String,
    /// The variant identifier.
    span: proc_macro2::Span,
}

fn event_variant(expression: &Expr) -> Option<EventVariant> {
    let path = match expression {
        Expr::Path(inner) => &inner.path,
        Expr::Struct(inner) => &inner.path,
        Expr::Call(call) => match &*call.func {
            Expr::Path(inner) => &inner.path,
            _ => return None,
        },
        Expr::Closure(closure) => return event_variant(&closure.body),
        Expr::Paren(inner) => return event_variant(&inner.expr),
        _ => return None,
    };
    let last = path.segments.last()?;
    let name = last.ident.to_string();
    let is_camel_case =
        name.chars().next().is_some_and(char::is_uppercase) && name.chars().any(char::is_lowercase);
    if path.segments.len() < 2 || !is_camel_case {
        return None; // a function, a constant or an import: unsure
    }
    let text = path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::");
    Some(EventVariant {
        name,
        path: text,
        span: last.ident.span(),
    })
}
