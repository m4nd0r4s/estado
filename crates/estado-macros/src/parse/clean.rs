//! Recognizing and stripping the machine's marker attributes. The emitted
//! types must not carry them: they are not real attributes (ADR 0010).

use syn::{Attribute, Fields, ItemEnum, ItemImpl, ItemStruct};

use crate::naming::facade_name;

/// Marker attributes consumed by `#[machine]`.
const MARKERS: [&str; 11] = [
    "root",
    "state",
    "handlers",
    "initial",
    "terminal",
    "substates",
    "region",
    "history",
    "after",
    "invoke",
    "unhandled",
];

/// The marker name of an attribute (`"initial"`, ..), or `None` when it is
/// an attribute of someone else (`derive`, `doc`, ..).
pub fn marker_name(attribute: &Attribute) -> Option<&'static str> {
    let written = facade_name(attribute.path())?;
    MARKERS.iter().copied().find(|marker| written == marker)
}

/// First attribute with the given marker name.
pub fn find_marker<'a>(attributes: &'a [Attribute], name: &str) -> Option<&'a Attribute> {
    attributes
        .iter()
        .find(|attribute| marker_name(attribute) == Some(name))
}

/// Whether the marker is present.
pub fn has_marker(attributes: &[Attribute], name: &str) -> bool {
    find_marker(attributes, name).is_some()
}

/// Removes every marker attribute from a list.
pub fn strip_markers(attributes: &mut Vec<Attribute>) {
    attributes.retain(|attribute| marker_name(attribute).is_none());
}

fn strip_fields(fields: &mut Fields) {
    for field in fields.iter_mut() {
        strip_markers(&mut field.attrs);
    }
}

/// An enum without machine attributes, on the item, variants and fields.
pub fn clean_enum(declaration: &ItemEnum) -> ItemEnum {
    let mut cleaned = declaration.clone();
    strip_markers(&mut cleaned.attrs);
    for variant in &mut cleaned.variants {
        strip_markers(&mut variant.attrs);
        strip_fields(&mut variant.fields);
    }
    cleaned
}

/// A struct without machine attributes, on the item and fields.
pub fn clean_struct(declaration: &ItemStruct) -> ItemStruct {
    let mut cleaned = declaration.clone();
    strip_markers(&mut cleaned.attrs);
    strip_fields(&mut cleaned.fields);
    cleaned
}

/// An impl block without `#[handlers]`; its content is untouched until the
/// handler generator runs.
pub fn clean_handlers(declaration: &ItemImpl) -> ItemImpl {
    let mut cleaned = declaration.clone();
    strip_markers(&mut cleaned.attrs);
    cleaned
}
