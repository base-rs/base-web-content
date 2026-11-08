// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! The `#[content(...)]` attributes of structs, fields, and enumeration variants.

use base_web_media_type::MediaRange;
use proc_macro2::Span;
use syn::meta::ParseNestedMeta;
use syn::punctuated::Punctuated;
use syn::{Attribute, Error, Expr, Ident, LitStr, Token, parenthesized};

/// The limits a struct may override, named as `base_web_content::__private::Limits` names them.
const STRUCT_LIMITS: &[&str] = &[
    "depth_limit",
    "content_size_limit",
    "part_count_limit",
    "header_size_limit",
    "header_count_limit",
    "preamble_size_limit",
    "epilogue_size_limit",
    "pair_count_limit",
    "url_encoded_size_limit",
];

/// The struct-level attributes.
#[derive(Default)]
pub(crate) struct StructAttributes {
    /// The limits the struct overrides, by name.
    pub limits: Vec<(Ident, Expr)>,
    /// The formats the struct accepts, if it restricts them: `(multipart/form-data, application/x-www-form-urlencoded)`.
    pub formats: Option<(bool, bool)>,
}

/// The attributes of a field.
#[derive(Default)]
pub(crate) struct FieldAttributes {
    pub rename: Option<LitStr>,
    pub size_limit: Option<(Span, Expr)>,
    pub count_limit: Option<(Span, Expr)>,
    /// The accepted media ranges, lowercase and without parameters.
    pub accept: Option<(Span, Vec<String>)>,
}

/// The attributes of an enumeration's variant.
#[derive(Default)]
pub(crate) struct VariantAttributes {
    pub rename: Option<LitStr>,
}

/// Call `handle` for each item in every `#[content(...)]` attribute.
fn each_meta(
    attributes: &[Attribute],
    mut handle: impl FnMut(ParseNestedMeta<'_>) -> syn::Result<()>,
) -> syn::Result<()> {
    for attribute in attributes {
        if attribute.path().is_ident("content") {
            attribute.parse_nested_meta(&mut handle)?;
        }
    }
    Ok(())
}

/// Set an attribute that may be given once.
fn once<T>(slot: &mut Option<T>, value: T, meta: &ParseNestedMeta<'_>) -> syn::Result<()> {
    if slot.is_some() {
        return Err(meta.error("this attribute is given more than once"));
    }
    *slot = Some(value);
    Ok(())
}

/// A parenthesized, non-empty list of string literals, such as `("image/png", "image/*")`.
fn strings(meta: &ParseNestedMeta<'_>) -> syn::Result<Vec<LitStr>> {
    let content;
    parenthesized!(content in meta.input);
    let list = Punctuated::<LitStr, Token![,]>::parse_terminated(&content)?;
    if list.is_empty() {
        return Err(meta.error("this attribute needs at least one value"));
    }
    Ok(list.into_iter().collect())
}

fn unknown(meta: &ParseNestedMeta<'_>, known: &str) -> Error {
    meta.error(format!("unknown `content` attribute; expected {known}"))
}

impl StructAttributes {
    pub(crate) fn parse(attributes: &[Attribute]) -> syn::Result<Self> {
        let mut parsed = Self::default();
        each_meta(attributes, |meta| {
            if meta.path.is_ident("formats") {
                let formats = strings(&meta)?;
                let mut accepted = (false, false);
                for format in &formats {
                    let flag = match format.value().as_str() {
                        "multipart/form-data" => &mut accepted.0,
                        "application/x-www-form-urlencoded" => &mut accepted.1,
                        _ => {
                            return Err(Error::new(
                                format.span(),
                                "unsupported format; expected `multipart/form-data` or `application/x-www-form-urlencoded`",
                            ));
                        }
                    };
                    if *flag {
                        return Err(Error::new(format.span(), "this format is listed twice"));
                    }
                    *flag = true;
                }
                return once(&mut parsed.formats, accepted, &meta);
            }
            let Some(name) = meta
                .path
                .get_ident()
                .filter(|name| STRUCT_LIMITS.iter().any(|limit| name == limit))
            else {
                return Err(unknown(
                    &meta,
                    &format!("`formats` or one of {}", STRUCT_LIMITS.join(", ")),
                ));
            };
            if parsed.limits.iter().any(|(limit, _)| limit == name) {
                return Err(meta.error("this attribute is given more than once"));
            }
            let name = name.clone();
            parsed.limits.push((name, meta.value()?.parse()?));
            Ok(())
        })?;
        Ok(parsed)
    }
}

impl FieldAttributes {
    pub(crate) fn parse(attributes: &[Attribute]) -> syn::Result<Self> {
        let mut parsed = Self::default();
        each_meta(attributes, |meta| {
            let span = meta.path.require_ident()?.span();
            if meta.path.is_ident("rename") {
                let name: LitStr = meta.value()?.parse()?;
                check_name(&name)?;
                once(&mut parsed.rename, name, &meta)
            } else if meta.path.is_ident("size_limit") {
                let limit = meta.value()?.parse()?;
                once(&mut parsed.size_limit, (span, limit), &meta)
            } else if meta.path.is_ident("count_limit") {
                let limit = meta.value()?.parse()?;
                once(&mut parsed.count_limit, (span, limit), &meta)
            } else if meta.path.is_ident("accept") {
                let ranges = strings(&meta)?
                    .iter()
                    .map(media_range)
                    .collect::<syn::Result<_>>()?;
                once(&mut parsed.accept, (span, ranges), &meta)
            } else {
                Err(unknown(
                    &meta,
                    "`rename`, `size_limit`, `count_limit`, or `accept`",
                ))
            }
        })?;
        Ok(parsed)
    }
}

impl VariantAttributes {
    pub(crate) fn parse(attributes: &[Attribute]) -> syn::Result<Self> {
        let mut parsed = Self::default();
        each_meta(attributes, |meta| {
            if !meta.path.is_ident("rename") {
                return Err(unknown(&meta, "`rename`"));
            }
            let spelling = meta.value()?.parse()?;
            once(&mut parsed.rename, spelling, &meta)
        })?;
        Ok(parsed)
    }
}

/// Check that a submitted name a field is renamed to is one a form can use: non-empty, without brackets, which separate a name's components.
fn check_name(name: &LitStr) -> syn::Result<()> {
    let value = name.value();
    if value.is_empty() || value.contains(['[', ']']) {
        return Err(Error::new(
            name.span(),
            "a field's name must be non-empty and contain no brackets; nested groups and arrays add them",
        ));
    }
    Ok(())
}

/// A media range for `accept`, lowercase and without parameters.
fn media_range(literal: &LitStr) -> syn::Result<String> {
    let range = MediaRange::parse(&literal.value()).map_err(|_| {
        Error::new(
            literal.span(),
            "`accept` needs media ranges, such as `image/png` or `image/*`",
        )
    })?;
    if range.parameters().len() > 0 {
        return Err(Error::new(
            literal.span(),
            "`accept` matches media types without their parameters, so a range cannot have any",
        ));
    }
    Ok(range.essence().to_owned())
}
