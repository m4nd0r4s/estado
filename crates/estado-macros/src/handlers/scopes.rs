//! Lexical scopes of local bindings, for the E0046 check: a name that a `let`,
//! a closure parameter, a match arm or an `if let` binds again no longer
//! refers to the parameter of the handler it shares its name with.

use syn::{Ident, Pat};

/// A stack of scopes, each holding the names bound in it.
pub struct Scopes {
    frames: Vec<Vec<Ident>>,
}

impl Scopes {
    /// One empty, outermost scope.
    pub fn new() -> Self {
        Self {
            frames: vec![Vec::new()],
        }
    }

    /// Enters a nested scope.
    pub fn push(&mut self) {
        self.frames.push(Vec::new());
    }

    /// Leaves the innermost scope, forgetting its bindings.
    pub fn pop(&mut self) {
        self.frames.pop();
    }

    /// Binds every name of `pattern` in the innermost scope.
    pub fn bind(&mut self, pattern: &Pat) {
        let frame = self.frames.last_mut().expect("the outermost scope stays");
        collect_bound(pattern, frame);
    }

    /// Whether `ident` is bound by an enclosing scope.
    pub fn shadows(&self, ident: &Ident) -> bool {
        self.frames.iter().any(|frame| frame.contains(ident))
    }
}

fn collect_bound(pattern: &Pat, bound: &mut Vec<Ident>) {
    match pattern {
        Pat::Ident(inner) => {
            bound.push(inner.ident.clone());
            if let Some((_, subpattern)) = &inner.subpat {
                collect_bound(subpattern, bound);
            }
        }
        Pat::Type(inner) => collect_bound(&inner.pat, bound),
        Pat::Reference(inner) => collect_bound(&inner.pat, bound),
        Pat::Paren(inner) => collect_bound(&inner.pat, bound),
        Pat::Tuple(inner) => inner.elems.iter().for_each(|pat| collect_bound(pat, bound)),
        Pat::Slice(inner) => inner.elems.iter().for_each(|pat| collect_bound(pat, bound)),
        Pat::Or(inner) => inner.cases.iter().for_each(|pat| collect_bound(pat, bound)),
        Pat::TupleStruct(inner) => inner.elems.iter().for_each(|pat| collect_bound(pat, bound)),
        Pat::Struct(inner) => inner
            .fields
            .iter()
            .for_each(|field| collect_bound(&field.pat, bound)),
        _ => {}
    }
}
