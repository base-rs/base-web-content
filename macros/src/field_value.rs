// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! `#[derive(FieldValue)]`: an enumeration converted from the exact spelling of one of its variants.

use crate::attributes::VariantAttributes;
use proc_macro2::TokenStream;
use quote::quote;
use syn::ext::IdentExt;
use syn::{Data, DeriveInput, Error, Fields, LitStr};

pub(crate) fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    let Data::Enum(data) = &input.data else {
        return Err(Error::new_spanned(
            &input.ident,
            "FieldValue can only be derived for an enumeration of unit variants",
        ));
    };
    if !input.generics.params.is_empty() || input.generics.where_clause.is_some() {
        return Err(Error::new_spanned(
            &input.generics,
            "FieldValue cannot be derived for a generic enumeration",
        ));
    }
    if let Some(attribute) = input
        .attrs
        .iter()
        .find(|attribute| attribute.path().is_ident("content"))
    {
        return Err(Error::new_spanned(
            attribute,
            "an enumeration takes no `content` attributes; give a variant's spelling with `#[content(rename = \"...\")]` on the variant",
        ));
    }
    if data.variants.is_empty() {
        return Err(Error::new_spanned(
            &input.ident,
            "FieldValue needs at least one variant to convert to",
        ));
    }
    let mut spellings: Vec<LitStr> = Vec::new();
    let mut variants = Vec::new();
    for variant in &data.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(Error::new_spanned(
                &variant.fields,
                "FieldValue can only be derived for an enumeration of unit variants",
            ));
        }
        let spelling = VariantAttributes::parse(&variant.attrs)?
            .rename
            .unwrap_or_else(|| {
                LitStr::new(&variant.ident.unraw().to_string(), variant.ident.span())
            });
        if spellings
            .iter()
            .any(|existing| existing.value() == spelling.value())
        {
            return Err(Error::new(
                spelling.span(),
                format!(
                    "the spelling `{}` is already used by another variant",
                    spelling.value()
                ),
            ));
        }
        spellings.push(spelling);
        variants.push(&variant.ident);
    }
    let name = &input.ident;
    Ok(quote! {
        #[automatically_derived]
        impl ::base_web_content::FieldValue for #name {
            fn from_field_text(text: &str) -> ::core::result::Result<Self, ::base_web_content::FieldValueError> {
                match text {
                    #(#spellings => ::core::result::Result::Ok(Self::#variants),)*
                    _ => ::core::result::Result::Err(::base_web_content::FieldValueError),
                }
            }
        }
    })
}
