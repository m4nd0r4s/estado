//! The reference pages (`docs/reference/`), compiled and run as doctests.

#[doc = include_str!("../../../docs/reference/README.md")]
pub mod index {}

#[doc = include_str!("../../../docs/reference/attributes.md")]
pub mod attributes {}

#[doc = include_str!("../../../docs/reference/macros.md")]
pub mod macros {}

#[doc = include_str!("../../../docs/reference/handlers.md")]
pub mod handlers {}

#[doc = include_str!("../../../docs/reference/machine.md")]
pub mod machine {}

#[doc = include_str!("../../../docs/reference/context.md")]
pub mod context {}

#[doc = include_str!("../../../docs/reference/effects.md")]
pub mod effects {}

#[doc = include_str!("../../../docs/reference/errors.md")]
pub mod errors {}

#[doc = include_str!("../../../docs/reference/configuration.md")]
pub mod configuration {}

#[doc = include_str!("../../../docs/reference/testing.md")]
pub mod testing {}

#[doc = include_str!("../../../docs/reference/core.md")]
pub mod core_api {}
