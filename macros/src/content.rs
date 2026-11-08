// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! `#[derive(Content)]`: a struct's declaration, and the code that builds it from a parsed body.

use crate::attributes::{FieldAttributes, StructAttributes};
use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::ext::IdentExt;
use syn::spanned::Spanned;
use syn::{
    Data, DataStruct, DeriveInput, Error, Fields, GenericArgument, Ident, LitStr, PathArguments,
    Type,
};

/// How a field holds its items.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Container {
    Required,
    Optional,
    Array,
    OptionalArray,
}

impl Container {
    fn is_array(self) -> bool {
        matches!(self, Self::Array | Self::OptionalArray)
    }
}

/// A declared field.
struct Field<'a> {
    member: &'a Ident,
    name: LitStr,
    container: Container,
    /// The type of one item: the field's type within `Option` or `Vec`.
    item: &'a Type,
    attributes: FieldAttributes,
}

pub(crate) fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    let Data::Struct(DataStruct {
        fields: Fields::Named(fields),
        ..
    }) = &input.data
    else {
        return Err(Error::new_spanned(
            &input.ident,
            "Content can only be derived for a struct with named fields",
        ));
    };
    if !input.generics.params.is_empty() || input.generics.where_clause.is_some() {
        return Err(Error::new_spanned(
            &input.generics,
            "Content cannot be derived for a generic struct",
        ));
    }
    let attributes = StructAttributes::parse(&input.attrs)?;
    let mut declared: Vec<Field<'_>> = Vec::new();
    for field in &fields.named {
        let member = field.ident.as_ref().expect("named fields have identifiers");
        let attributes = FieldAttributes::parse(&field.attrs)?;
        let name = attributes
            .rename
            .clone()
            .unwrap_or_else(|| LitStr::new(&member.unraw().to_string(), member.span()));
        if let Some(existing) = declared
            .iter()
            .find(|existing| existing.name.value() == name.value())
        {
            return Err(Error::new(
                name.span(),
                format!(
                    "the name `{}` is already declared by the field `{}`",
                    name.value(),
                    existing.member
                ),
            ));
        }
        let (container, item) = classify(&field.ty)?;
        if names_type(item, &input.ident) {
            return Err(Error::new_spanned(
                item,
                "a declaration cannot contain itself, since its names would have no depth limit",
            ));
        }
        if let (Some((span, _)), false) = (&attributes.count_limit, container.is_array()) {
            return Err(Error::new(
                *span,
                "`count_limit` applies to an array, a `Vec` field",
            ));
        }
        declared.push(Field {
            member,
            name,
            container,
            item,
            attributes,
        });
    }
    Ok(generate(&input.ident, &attributes, &declared))
}

/// The container a field's type declares, and the type of one item, as the field's type spells them: `Option<T>`, `Vec<T>`, `Option<Vec<T>>`, or `T`, where `Vec<u8>` is bytes rather than an array.
fn classify(ty: &Type) -> syn::Result<(Container, &Type)> {
    let (optional, ty) = match wrapped(ty, "Option") {
        Some(inner) => (true, inner),
        None => (false, ty),
    };
    if optional && wrapped(ty, "Option").is_some() {
        return Err(Error::new_spanned(
            ty,
            "a field is optional once: `Option` cannot contain another `Option`",
        ));
    }
    let Some(element) = array_element(ty) else {
        let container = if optional {
            Container::Optional
        } else {
            Container::Required
        };
        return Ok((container, ty));
    };
    if wrapped(element, "Option").is_some() {
        return Err(Error::new_spanned(
            element,
            "an array's elements cannot be optional",
        ));
    }
    if array_element(element).is_some() {
        return Err(Error::new_spanned(
            element,
            "an array's elements cannot be arrays",
        ));
    }
    let container = if optional {
        Container::OptionalArray
    } else {
        Container::Array
    };
    Ok((container, element))
}

/// The type within `wrapper<T>`, when `ty` is spelled that way.
fn wrapped<'a>(ty: &'a Type, wrapper: &str) -> Option<&'a Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path
        .path
        .segments
        .last()
        .expect("a parsed path has a segment");
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    match (
        segment.ident == wrapper,
        arguments.args.first(),
        arguments.args.len(),
    ) {
        (true, Some(GenericArgument::Type(inner)), 1) => Some(inner),
        _ => None,
    }
}

/// Whether `ty` is spelled as the struct being declared, by its name or as `Self`.
fn names_type(ty: &Type, name: &Ident) -> bool {
    matches!(ty, Type::Path(path) if path.qself.is_none() && (path.path.is_ident(name) || path.path.is_ident("Self")))
}

/// The element type of an array, `Vec<T>` for any `T` but `u8`, however its path is written, such as `core::primitive::u8`, since `Vec<u8>` is bytes. An alias of `u8` is not seen through.
fn array_element(ty: &Type) -> Option<&Type> {
    wrapped(ty, "Vec").filter(|element| !is_byte(element))
}

fn is_byte(ty: &Type) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };
    let segment = path
        .path
        .segments
        .last()
        .expect("a parsed path has a segment");
    path.qself.is_none() && segment.ident == "u8" && segment.arguments.is_none()
}

fn generate(name: &Ident, attributes: &StructAttributes, fields: &[Field<'_>]) -> TokenStream {
    let declarations = fields.iter().map(declaration);
    let checks = fields.iter().map(check);
    let limits = attributes
        .limits
        .iter()
        .map(|(limit, value)| quote!(#limit: #value,));
    let (multipart_form_data, url_encoded) = attributes.formats.unwrap_or((true, true));
    // Each bound is spanned on its type, so an unsupported type is reported once, at its field.
    let bounds = fields.iter().map(|field| {
        let item = field.item;
        quote_spanned!(item.span() => #item: __content::Item<__H>,)
    });
    let fields_variable = Ident::new("__fields", Span::mixed_site());
    let depth_message = format!("`{name}` declares names deeper than its depth limit");
    let builds = fields.iter().map(|field| {
        let (member, item) = (field.member, field.item);
        let container = match field.container {
            Container::Required => quote!(required),
            Container::Optional => quote!(optional),
            Container::Array => quote!(array),
            Container::OptionalArray => quote!(optional_array),
        };
        quote_spanned!(item.span() => #member: __content::#container::<__H, #item>(&mut #fields_variable),)
    });
    quote! {
        const _: () = {
            use ::base_web_content::__private as __content;

            #[automatically_derived]
            impl __content::Declaration for #name {
                const DECLARATION: __content::ContentDeclaration = __content::ContentDeclaration {
                    group: &__content::GroupDeclaration {
                        fields: &[#(#declarations)*],
                    },
                    limits: __content::Limits {
                        #(#limits)*
                        ..__content::Limits::DEFAULT
                    },
                    formats: __content::Formats {
                        multipart_form_data: #multipart_form_data,
                        url_encoded: #url_encoded,
                    },
                };
            }

            const _: () = {
                let declaration = <#name as __content::Declaration>::DECLARATION;
                ::core::assert!(__content::depth(declaration.group) <= declaration.limits.depth_limit, #depth_message);
            };
            #(#checks)*

            #[automatically_derived]
            impl __content::ItemDeclaration for #name {
                const KIND: __content::ItemKind = __content::ItemKind::Group(<#name as __content::Declaration>::DECLARATION.group);
            }

            #[automatically_derived]
            impl<__H> __content::Item<__H> for #name where #(#bounds)* {
                fn build(value: __content::CheckedValue<__H>) -> Self {
                    let mut #fields_variable = __content::group_fields(value);
                    Self { #(#builds)* }
                }
            }

            #[automatically_derived]
            impl ::base_web_content::Content for #name {}
        };
    }
}

/// A field's `FieldDeclaration`.
fn declaration(field: &Field<'_>) -> TokenStream {
    let (name, item) = (&field.name, field.item);
    let count_limit = field.attributes.count_limit.as_ref().map_or_else(
        || quote!(__content::COUNT_LIMIT),
        |(_, limit)| quote!(#limit),
    );
    let container = match field.container {
        Container::Required => quote!(Required),
        Container::Optional => quote!(Optional),
        Container::Array => quote!(Array(#count_limit)),
        Container::OptionalArray => quote!(OptionalArray(#count_limit)),
    };
    let size_limit = match &field.attributes.size_limit {
        Some((_, limit)) => quote!(::core::option::Option::Some(#limit)),
        None => quote!(::core::option::Option::None),
    };
    let accept = field
        .attributes
        .accept
        .iter()
        .flat_map(|(_, ranges)| ranges);
    let kind = quote_spanned!(item.span() => <#item as __content::ItemDeclaration>::KIND);
    quote! {
        __content::FieldDeclaration {
            name: #name,
            kind: #kind,
            container: __content::Container::#container,
            size_limit: #size_limit,
            accept: &[#(#accept),*],
        },
    }
}

/// Checks, when the declaration is compiled, that a field's attributes suit its type, which only the compiler knows.
fn check(field: &Field<'_>) -> TokenStream {
    let item = field.item;
    let kind = quote!(<#item as __content::ItemDeclaration>::KIND);
    let mut checks = TokenStream::new();
    if let Some((span, _)) = &field.attributes.size_limit {
        let message = format!(
            "`size_limit` on `{}` applies to a scalar or an upload, not a nested group",
            field.member
        );
        checks.extend(quote_spanned!(*span => const _: () = ::core::assert!(!::core::matches!(#kind, __content::ItemKind::Group(_)), #message);));
    }
    if let Some((span, limit)) = &field.attributes.count_limit {
        let message = format!("`count_limit` on `{}` must be at least 1", field.member);
        // Written as a condition rather than `assert!`, which Clippy would report as a constant assertion in the application's code.
        checks.extend(
            quote_spanned!(*span => const _: () = if #limit == 0 { ::core::panic!(#message) };),
        );
    }
    if let Some((span, _)) = &field.attributes.accept {
        let message = format!("`accept` on `{}` applies to an upload", field.member);
        checks.extend(quote_spanned!(*span => const _: () = ::core::assert!(::core::matches!(#kind, __content::ItemKind::Upload), #message);));
    }
    checks
}
