//! Walks the items of the `#[machine]` module and sorts them by role.

use syn::spanned::Spanned;
use syn::{Item, ItemEnum, ItemImpl, ItemStruct};

use super::clean::{
    clean_enum, clean_handlers, clean_struct, find_marker, has_marker, strip_markers,
};
use crate::model::Diagnostic;

/// A type that takes part in the machine, as written (attributes intact).
pub enum RawType {
    /// `#[root]` enum.
    Root(ItemEnum),
    /// `#[state]` enum.
    StateEnum(ItemEnum),
    /// `#[state]` struct.
    StateStruct(ItemStruct),
}

/// The items of the module, by role.
#[derive(Default)]
pub struct ModuleItems {
    /// Root and state types in declaration order.
    pub types: Vec<RawType>,
    /// `#[handlers]` impl blocks.
    pub handlers: Vec<ItemImpl>,
    /// Everything else.
    pub passthrough: Vec<Item>,
}

impl ModuleItems {
    /// Every item again, markers removed, for a module that has no usable
    /// root (types first, then handlers and the rest).
    pub fn into_cleaned_items(self) -> Vec<Item> {
        let types = self.types.iter().map(|raw| match raw {
            RawType::Root(declaration) | RawType::StateEnum(declaration) => {
                Item::Enum(clean_enum(declaration))
            }
            RawType::StateStruct(declaration) => Item::Struct(clean_struct(declaration)),
        });
        let handlers = self
            .handlers
            .iter()
            .map(|declaration| Item::Impl(clean_handlers(declaration)));
        types
            .chain(handlers)
            .chain(self.passthrough.iter().cloned())
            .collect()
    }

    /// The `#[root]` enums, in declaration order.
    pub fn roots(&self) -> Vec<&ItemEnum> {
        self.types
            .iter()
            .filter_map(|raw| match raw {
                RawType::Root(declaration) => Some(declaration),
                _ => None,
            })
            .collect()
    }

    /// A `#[state]` enum by name.
    pub fn state_enum(&self, name: &syn::Ident) -> Option<&ItemEnum> {
        self.types.iter().find_map(|raw| match raw {
            RawType::StateEnum(declaration) if declaration.ident == *name => Some(declaration),
            _ => None,
        })
    }

    /// A `#[state]` struct by name.
    pub fn state_struct(&self, name: &syn::Ident) -> Option<&ItemStruct> {
        self.types.iter().find_map(|raw| match raw {
            RawType::StateStruct(declaration) if declaration.ident == *name => Some(declaration),
            _ => None,
        })
    }

    /// A plain enum (without `#[state]`) by name, to suggest marking it.
    pub fn unmarked_enum(&self, name: &syn::Ident) -> Option<&ItemEnum> {
        self.passthrough.iter().find_map(|item| match item {
            Item::Enum(declaration) if declaration.ident == *name => Some(declaration),
            _ => None,
        })
    }
}

/// Sorts the module items by role. Marker attributes on the wrong kind of
/// item are reported (E0001 for `#[root]`, E0017 for the others) and the item
/// is kept as a passthrough with the marker removed.
pub fn sort_items(items: Vec<Item>) -> (ModuleItems, Vec<Diagnostic>) {
    let mut sorted = ModuleItems::default();
    let mut diagnostics = Vec::new();
    for item in items {
        match item {
            Item::Enum(declaration) if has_marker(&declaration.attrs, "root") => {
                sorted.types.push(RawType::Root(declaration));
            }
            Item::Enum(declaration) if has_marker(&declaration.attrs, "state") => {
                sorted.types.push(RawType::StateEnum(declaration));
            }
            Item::Struct(declaration) if has_marker(&declaration.attrs, "state") => {
                sorted.types.push(RawType::StateStruct(declaration));
            }
            Item::Impl(declaration) if has_marker(&declaration.attrs, "handlers") => {
                sorted.handlers.push(declaration);
            }
            mut other => {
                misplaced_marker(&mut other, &mut diagnostics);
                sorted.passthrough.push(other);
            }
        }
    }
    (sorted, diagnostics)
}

fn item_attributes_mut(item: &mut Item) -> Option<&mut Vec<syn::Attribute>> {
    Some(match item {
        Item::Enum(inner) => &mut inner.attrs,
        Item::Struct(inner) => &mut inner.attrs,
        Item::Impl(inner) => &mut inner.attrs,
        Item::Fn(inner) => &mut inner.attrs,
        Item::Union(inner) => &mut inner.attrs,
        Item::Type(inner) => &mut inner.attrs,
        Item::Trait(inner) => &mut inner.attrs,
        Item::Mod(inner) => &mut inner.attrs,
        Item::Const(inner) => &mut inner.attrs,
        Item::Static(inner) => &mut inner.attrs,
        _ => return None,
    })
}

/// Reports `#[root]`/`#[state]`/`#[handlers]` on an item that cannot carry
/// it, and removes them so the standalone attribute does not fire as well.
fn misplaced_marker(item: &mut Item, diagnostics: &mut Vec<Diagnostic>) {
    let Some(attributes) = item_attributes_mut(item) else {
        return;
    };
    if let Some(attribute) = find_marker(attributes, "root") {
        diagnostics.push(
            Diagnostic::new("E0001", attribute.span(), "`#[root]` must be on an enum")
                .with_help("the machine is the enum of its top-level states"),
        );
    }
    for (marker, expected) in [
        ("state", "a struct or an enum"),
        ("handlers", "an `impl` block"),
    ] {
        if let Some(attribute) = find_marker(attributes, marker) {
            diagnostics.push(Diagnostic::new(
                "E0017",
                attribute.span(),
                format!("`#[{marker}]` must be on {expected}"),
            ));
        }
    }
    strip_markers(attributes);
}
