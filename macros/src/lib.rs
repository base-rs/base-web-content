// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! The derive macros for `base-web-content` declarations. Use them through `base_web_content`, rather than depending on this crate directly.

use proc_macro::TokenStream;
use syn::{DeriveInput, Error};

mod attributes;
mod content;
mod field_value;

#[cfg(test)]
mod tests;

/// Derive a declaration of the content a struct accepts, from its named fields.
///
/// See `base_web_content::Content` for the field types and attributes it accepts.
#[proc_macro_derive(Content, attributes(content))]
pub fn derive_content(input: TokenStream) -> TokenStream {
    // A derive's input always parses as an item, so errors come from the declarations.
    syn::parse::<DeriveInput>(input)
        .and_then(|input| content::expand(&input))
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

/// Derive the conversion of an enumeration from a field's text: the exact, case-sensitive spelling of each variant.
///
/// See `base_web_content::FieldValue` for the spellings and attributes it accepts.
#[proc_macro_derive(FieldValue, attributes(content))]
pub fn derive_field_value(input: TokenStream) -> TokenStream {
    syn::parse::<DeriveInput>(input)
        .and_then(|input| field_value::expand(&input))
        .unwrap_or_else(Error::into_compile_error)
        .into()
}
