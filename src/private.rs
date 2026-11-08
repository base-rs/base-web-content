// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! The items that the code `#[derive(Content)]` and `#[derive(FieldValue)]` generate refers to. They are not public API and may change in any release.

pub use crate::declaration::{
    COUNT_LIMIT, Container, ContentDeclaration, FieldDeclaration, Formats, GroupDeclaration,
    ItemKind, Limits, depth,
};
use crate::form::CheckedSlot;
pub use crate::form::{CheckedValue, GroupFields};
use crate::{FieldValue, Upload, field_value::Charset};
use std::any::Any;

/// A type that can be one form field, or one element of an array.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be a field of a `Content` declaration",
    label = "not a field type",
    note = "a field is `String`, `Vec<u8>`, an integer, `f32`, `f64`, `bool`, a `FieldValue`, an `Upload`, or a struct deriving `Content`, in `Option` when it may be omitted, or `Vec` for an array"
)]
pub trait ItemDeclaration {
    const KIND: ItemKind;
}

/// An item built from a body's checked values, with `H` the caller's upload handles.
pub trait Item<H>: ItemDeclaration + Sized {
    fn build(value: CheckedValue<H>) -> Self;
}

/// A struct deriving `Content`, as the top level of a body.
pub trait Declaration {
    const DECLARATION: ContentDeclaration;
}

impl<T: FieldValue> ItemDeclaration for T {
    const KIND: ItemKind = ItemKind::Scalar(convert_text::<T>);
}

impl<H, T: FieldValue> Item<H> for T {
    fn build(value: CheckedValue<H>) -> Self {
        scalar(value)
    }
}

/// The bytes as they were submitted, in any character set.
impl ItemDeclaration for Vec<u8> {
    const KIND: ItemKind = ItemKind::Scalar(convert_bytes);
}

impl<H> Item<H> for Vec<u8> {
    fn build(value: CheckedValue<H>) -> Self {
        scalar(value)
    }
}

impl<H> ItemDeclaration for Upload<H> {
    const KIND: ItemKind = ItemKind::Upload;
}

impl<H> Item<H> for Upload<H> {
    fn build(value: CheckedValue<H>) -> Self {
        match value {
            CheckedValue::Upload(upload) => upload,
            _ => unreachable!("an upload field holds an upload"),
        }
    }
}

fn convert_text<T: FieldValue>(bytes: Vec<u8>, charset: Charset) -> Option<Box<dyn Any>> {
    let text = charset.decode(bytes)?;
    let value = T::from_field_text(&text).ok()?;
    Some(Box::new(value))
}

fn convert_bytes(bytes: Vec<u8>, _charset: Charset) -> Option<Box<dyn Any>> {
    Some(Box::new(bytes))
}

fn scalar<T: 'static, H>(value: CheckedValue<H>) -> T {
    match value {
        CheckedValue::Scalar(value) => *value
            .downcast()
            .expect("a scalar field holds the value its declaration converted"),
        _ => unreachable!("a scalar field holds a converted value"),
    }
}

/// The fields of a group, to build them in declaration order.
pub fn group_fields<H>(value: CheckedValue<H>) -> GroupFields<H> {
    match value {
        CheckedValue::Group(slots) => GroupFields::new(slots),
        _ => unreachable!("a nested group's field holds a group"),
    }
}

/// The next field, which is required.
pub fn required<H, T: Item<H>>(fields: &mut GroupFields<H>) -> T {
    match fields.next_slot() {
        CheckedSlot::Single(value) => T::build(value),
        _ => unreachable!("a required field that was checked has a value"),
    }
}

/// The next field, which may be omitted.
pub fn optional<H, T: Item<H>>(fields: &mut GroupFields<H>) -> Option<T> {
    match fields.next_slot() {
        CheckedSlot::Absent => None,
        CheckedSlot::Single(value) => Some(T::build(value)),
        CheckedSlot::Array(_) => unreachable!("an optional field holds one value"),
    }
}

/// The next field, a required array.
pub fn array<H, T: Item<H>>(fields: &mut GroupFields<H>) -> Vec<T> {
    match fields.next_slot() {
        CheckedSlot::Array(values) => values.into_iter().map(T::build).collect(),
        _ => unreachable!("a required array that was checked has elements"),
    }
}

/// The next field, an array that may be omitted.
pub fn optional_array<H, T: Item<H>>(fields: &mut GroupFields<H>) -> Option<Vec<T>> {
    match fields.next_slot() {
        CheckedSlot::Absent => None,
        CheckedSlot::Array(values) => Some(values.into_iter().map(T::build).collect()),
        CheckedSlot::Single(_) => unreachable!("an array field holds elements"),
    }
}
