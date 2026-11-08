// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! The fields of a form as they arrive, in any format: names matched against the declaration, values placed in a tree that mirrors it, and, when parsing ends, the tree checked and built into the declared struct.

use crate::__private::Item;
use crate::declaration::{ContentDeclaration, FieldDeclaration, GroupDeclaration, ItemKind};
use crate::field_value::Charset;
use crate::{
    Accepted, ContentError, ContentErrorKind, FieldPath, FieldPathSegment, Rejected, Rejection,
    Upload, UploadHead, Violation, ViolationCode,
};
use base_web_media_type::{MediaRange, MediaRangeSet, MediaType};
use base_web_url::{FormErrorKind, FormPath, FormPathSegment};
use std::any::Any;

#[cfg(test)]
mod tests;

/// A group's fields as they arrive, one slot for each declared field.
struct GroupValue<H> {
    slots: Vec<Slot<H>>,
}

enum Slot<H> {
    Empty,
    /// An upload whose part chose no file, which arrived but holds nothing.
    NoFile,
    Single(ItemValue<H>),
    /// An array's elements, which are none when only parts with no file chosen arrived.
    Array(Vec<ItemValue<H>>),
}

enum ItemValue<H> {
    /// A field whose content is arriving.
    Pending,
    Scalar(Vec<u8>, Charset),
    Upload(Upload<H>),
    Group(GroupValue<H>),
    /// A field that arrived but cannot be accepted, whose content was discarded.
    Invalid(ViolationCode),
}

/// The slot, and the element of an array, for each component of a name, from the top level.
type Location = Vec<(usize, Option<usize>)>;

impl<H> GroupValue<H> {
    fn new(declaration: &GroupDeclaration) -> Self {
        Self {
            slots: declaration.fields.iter().map(|_| Slot::Empty).collect(),
        }
    }
}

impl<H> Slot<H> {
    /// The group a nested field's name continues into, created as needed. For an array of groups, the last element unless it already has the field `next`, which begins a new element.
    fn group(
        &mut self,
        declaration: &GroupDeclaration,
        count_limit: Option<usize>,
        next: usize,
    ) -> Result<(&mut GroupValue<H>, Option<usize>), ContentErrorKind> {
        let Some(count_limit) = count_limit else {
            if let Self::Empty = self {
                *self = Self::Single(ItemValue::Group(GroupValue::new(declaration)));
            }
            return Ok((self.item(None).group(), None));
        };
        if let Self::Empty = self {
            *self = Self::Array(Vec::new());
        }
        let Self::Array(elements) = self else {
            unreachable!("an array's slot holds its elements")
        };
        // As `protocol-url`'s `Encoding.assign` does, an element ends when a field it already has arrives again.
        let reuse = elements
            .last_mut()
            .is_some_and(|element| matches!(element.group().slots[next], Slot::Empty));
        if !reuse {
            if elements.len() == count_limit {
                return Err(ContentErrorKind::CountLimit);
            }
            elements.push(ItemValue::Group(GroupValue::new(declaration)));
        }
        let index = elements.len() - 1;
        Ok((elements[index].group(), Some(index)))
    }

    /// Mark a scalar or upload as arriving, returning the element index for an array, and the handle of an upload it replaces.
    fn begin_item(
        &mut self,
        count_limit: Option<usize>,
    ) -> Result<(Option<usize>, Option<H>), ContentErrorKind> {
        let Some(count_limit) = count_limit else {
            // The last value of a repeated name wins.
            let replaced = std::mem::replace(self, Self::Single(ItemValue::Pending));
            let handle = match replaced {
                Self::Single(ItemValue::Upload(upload)) => Some(upload.into_handle()),
                _ => None,
            };
            return Ok((None, handle));
        };
        if let Self::Empty = self {
            *self = Self::Array(Vec::new());
        }
        let Self::Array(elements) = self else {
            unreachable!("an array's slot holds its elements")
        };
        if elements.len() == count_limit {
            return Err(ContentErrorKind::CountLimit);
        }
        elements.push(ItemValue::Pending);
        Ok((Some(elements.len() - 1), None))
    }

    /// Mark an upload whose part chose no file as arrived, so that it ends an element of an array of groups as any field does, without replacing an upload already there or adding an element to an array.
    fn choose_no_file(&mut self, array: bool) {
        if let Self::Empty = self {
            *self = if array {
                Self::Array(Vec::new())
            } else {
                Self::NoFile
            };
        }
    }

    /// The item at `element` of an array, or the single item when `element` is `None`.
    fn item(&mut self, element: Option<usize>) -> &mut ItemValue<H> {
        match (self, element) {
            (Self::Single(item), None) => item,
            (Self::Array(items), Some(index)) => &mut items[index],
            _ => unreachable!("a location follows the slots it was placed in"),
        }
    }
}

impl<H> ItemValue<H> {
    fn group(&mut self) -> &mut GroupValue<H> {
        match self {
            Self::Group(group) => group,
            _ => unreachable!("a nested group's item is a group"),
        }
    }
}

/// What a part or pair says about the field it carries.
pub(crate) enum FieldSource<'a> {
    /// A value, with the character set its part declared.
    Value(Charset),
    /// No value at all, as a URL-encoded pair without `=` has, which no field type accepts.
    Absent,
    /// A file, with the filename and media type its part declared.
    File {
        filename: &'a str,
        media_type: Option<&'a MediaType>,
    },
}

/// The field whose content is arriving.
struct Receiving {
    location: Location,
    size: usize,
    size_limit: usize,
    mode: Mode,
}

enum Mode {
    /// A scalar, buffered until it ends.
    Buffer(Vec<u8>, Charset),
    /// A field that cannot be accepted, whose content is read and discarded.
    Discard,
    /// An upload, reported to the caller.
    Upload(UploadHead),
    /// A part for an upload with an empty filename, which browsers send for a file input with no file chosen: nothing is placed, and the part must have no content.
    NoFile,
}

/// An upload that ended, waiting for the caller's handle.
struct Unstored {
    location: Location,
    head: UploadHead,
    size: usize,
}

/// The fields of a body as they arrive.
pub(crate) struct Form<H> {
    declaration: &'static ContentDeclaration,
    root: GroupValue<H>,
    /// The handles of uploads a later field replaced.
    discarded: Vec<H>,
    /// The content of every field so far.
    content_size: usize,
    receiving: Option<Receiving>,
    unstored: Option<Unstored>,
}

impl<H> Form<H> {
    pub(crate) fn new(declaration: &'static ContentDeclaration) -> Self {
        Self {
            declaration,
            root: GroupValue::new(declaration.group),
            discarded: Vec::new(),
            content_size: 0,
            receiving: None,
            unstored: None,
        }
    }

    /// A field begins, with its name. Returns the upload's head when it is an upload the caller must store.
    ///
    /// A name the declaration does not accept is an error before any content is read. A field that cannot be accepted, such as a file where text is declared, records a violation, and its content is discarded.
    pub(crate) fn begin(
        &mut self,
        name: &str,
        source: FieldSource<'_>,
    ) -> Result<Option<UploadHead>, ContentErrorKind> {
        // Names are read by the bracket grammar `base-web-url` gives URL-encoded names, so both formats read a name alike.
        let path =
            FormPath::parse(name, self.declaration.limits.depth_limit).map_err(
                |kind| match kind {
                    FormErrorKind::DepthLimit => ContentErrorKind::DepthLimit,
                    _ => ContentErrorKind::UnknownName,
                },
            )?;
        let components: Vec<&str> = path
            .segments()
            .map(|segment| match segment {
                FormPathSegment::Key(key) => key,
                FormPathSegment::Element => "",
            })
            .collect();
        let fields =
            resolve(self.declaration.group, &components).ok_or(ContentErrorKind::UnknownName)?;
        // An upload's part with an empty filename chose no file: it holds nothing, and `accept` does not apply to it.
        let no_file = matches!(
            (self.declared(&fields).kind, &source),
            (ItemKind::Upload, FieldSource::File { filename: "", .. })
        );
        let Some((location, path, field)) = self.place(&fields, no_file)? else {
            self.receiving = Some(Receiving {
                // There is no item for its content, which must be empty.
                location: Location::new(),
                size: 0,
                size_limit: 0,
                mode: Mode::NoFile,
            });
            return Ok(None);
        };
        let mode = match (field.kind, source) {
            (ItemKind::Scalar(_), FieldSource::Value(charset)) => Mode::Buffer(Vec::new(), charset),
            (
                ItemKind::Upload,
                FieldSource::File {
                    filename,
                    media_type,
                },
            ) => {
                if accepts(field.accept, media_type) {
                    Mode::Upload(UploadHead::new(
                        path,
                        filename.to_owned(),
                        media_type.cloned(),
                    ))
                } else {
                    self.invalidate(&location, ViolationCode::UnsupportedMediaType)
                }
            }
            // A file where a scalar is declared, a value where an upload is, or no value at all.
            _ => self.invalidate(&location, ViolationCode::InvalidType),
        };
        let head = match &mode {
            Mode::Upload(head) => Some(head.clone()),
            Mode::Buffer(..) | Mode::Discard | Mode::NoFile => None,
        };
        self.receiving = Some(Receiving {
            location,
            size: 0,
            size_limit: field.size_limit(),
            mode,
        });
        Ok(head)
    }

    /// Some of the current field's content. Returns how many bytes are within the field's size limit and the total content limit, and the error when not all of them are.
    pub(crate) fn content(&mut self, bytes: &[u8]) -> (usize, Option<ContentErrorKind>) {
        let (allowance, limit) = self.allowance();
        let accepted = bytes.len().min(allowance);
        let receiving = self
            .receiving
            .as_mut()
            .expect("content belongs to a field that began");
        receiving.size += accepted;
        self.content_size += accepted;
        if let Mode::Buffer(buffer, _) = &mut receiving.mode {
            buffer.extend_from_slice(&bytes[..accepted]);
        }
        (accepted, (accepted < bytes.len()).then_some(limit))
    }

    /// The bytes of content the current field may still have, within its size limit and the total content limit, and the limit that more would exceed: the field's, when both would be. A part with no file chosen may have none.
    pub(crate) fn allowance(&self) -> (usize, ContentErrorKind) {
        let receiving = self
            .receiving
            .as_ref()
            .expect("content belongs to a field that began");
        if let Mode::NoFile = receiving.mode {
            return (0, ContentErrorKind::EmptyFilenameContent);
        }
        let field_remaining = receiving.size_limit - receiving.size;
        let total_remaining = self.declaration.limits.content_size_limit - self.content_size;
        if field_remaining <= total_remaining {
            (field_remaining, ContentErrorKind::FieldSizeLimit)
        } else {
            (total_remaining, ContentErrorKind::ContentSizeLimit)
        }
    }

    /// The current field ends. Returns whether it is an upload, whose handle the caller must give with [`Self::store`].
    pub(crate) fn end(&mut self) -> bool {
        let receiving = self.receiving.take().expect("a field ends after it began");
        match receiving.mode {
            Mode::Buffer(bytes, charset) => {
                *self.item(&receiving.location) = ItemValue::Scalar(bytes, charset);
                false
            }
            Mode::Discard | Mode::NoFile => false,
            Mode::Upload(head) => {
                self.unstored = Some(Unstored {
                    location: receiving.location,
                    head,
                    size: receiving.size,
                });
                true
            }
        }
    }

    /// Whether a field's content is arriving.
    pub(crate) fn is_receiving(&self) -> bool {
        self.receiving.is_some()
    }

    /// Whether an upload ended and its handle has not been given.
    pub(crate) fn is_unstored(&self) -> bool {
        self.unstored.is_some()
    }

    /// The caller's handle for the upload that ended.
    ///
    /// # Panics
    ///
    /// When no upload is waiting for its handle.
    pub(crate) fn store(&mut self, handle: H) {
        let unstored = self
            .unstored
            .take()
            .expect("an upload ended and waits for its handle");
        let upload = Upload::from_head(unstored.head, unstored.size as u64, handle);
        *self.item(&unstored.location) = ItemValue::Upload(upload);
    }

    /// The declared struct, or why it was refused with every handle the caller gave. `error` is the fatal error parsing ended with, if any.
    // SECURITY: An upload whose content was still arriving when parsing failed, or that ended without the caller giving its handle, has no handle here, so it cannot be returned. This does not leak storage: the caller began storing it at `ContentEvent::Upload` and still holds that storage, since it never gave the parser a handle for it, and `ContentParser`'s documentation and the guide tell it to remove what it stored of an upload still arriving.
    pub(crate) fn finish<T: Item<H>>(
        self,
        error: Option<ContentError>,
    ) -> Result<Accepted<T, H>, Rejected<H>> {
        let mut handles = self.discarded;
        if let Some(error) = error {
            release_group(self.root, &mut handles);
            return Err(Rejected {
                reason: Rejection::Error(error),
                handles,
            });
        }
        let (mut path, mut violations) = (FieldPath::default(), Vec::new());
        let checked = check_group(
            self.declaration.group,
            self.root,
            &mut path,
            &mut violations,
        );
        if violations.is_empty() {
            return Ok(Accepted {
                content: T::build(CheckedValue::Group(checked)),
                discarded: handles,
            });
        }
        release_checked(checked, &mut handles);
        Err(Rejected {
            reason: Rejection::Violations(violations),
            handles,
        })
    }

    /// Place a resolved name in the tree, creating its groups and array elements, and mark its item as arriving, or, for an upload with `no_file` chosen, as arrived with nothing to place.
    fn place(
        &mut self,
        fields: &[usize],
        no_file: bool,
    ) -> Result<Option<(Location, FieldPath, &'static FieldDeclaration)>, ContentErrorKind> {
        let (mut location, mut path) = (Location::new(), FieldPath::default());
        let (mut declaration, mut group) = (self.declaration.group, &mut self.root);
        for (depth, &index) in fields.iter().enumerate() {
            let field = &declaration.fields[index];
            let count_limit = field.container.count_limit();
            path.push(FieldPathSegment::Name(field.name));
            let element = match (field.kind, fields.get(depth + 1)) {
                (ItemKind::Group(inner), Some(&next)) => {
                    let (inner_group, element) =
                        group.slots[index].group(inner, count_limit, next)?;
                    group = inner_group;
                    declaration = inner;
                    element
                }
                _ if no_file => {
                    group.slots[index].choose_no_file(count_limit.is_some());
                    return Ok(None);
                }
                _ => {
                    let (element, replaced) = group.slots[index].begin_item(count_limit)?;
                    self.discarded.extend(replaced);
                    location.push((index, element));
                    path.extend(element.map(FieldPathSegment::Index));
                    return Ok(Some((location, path, field)));
                }
            };
            location.push((index, element));
            path.extend(element.map(FieldPathSegment::Index));
        }
        unreachable!("a resolved name ends at a scalar or an upload")
    }

    /// The declaration of the scalar or upload a resolved name ends at.
    fn declared(&self, fields: &[usize]) -> &'static FieldDeclaration {
        let (last, parents) = fields.split_last().expect("a resolved name is not empty");
        let mut group = self.declaration.group;
        for &index in parents {
            let ItemKind::Group(inner) = group.fields[index].kind else {
                unreachable!("a resolved name continues only into nested groups")
            };
            group = inner;
        }
        &group.fields[*last]
    }

    fn item(&mut self, location: &Location) -> &mut ItemValue<H> {
        let ((index, element), parents) = location.split_last().expect("a location is not empty");
        let mut group = &mut self.root;
        for &(index, element) in parents {
            group = group.slots[index].item(element).group();
        }
        group.slots[*index].item(*element)
    }

    /// Record that the field at `location` cannot be accepted, and discard its content.
    fn invalidate(&mut self, location: &Location, code: ViolationCode) -> Mode {
        *self.item(location) = ItemValue::Invalid(code);
        Mode::Discard
    }
}

/// The index of the field for each name component of a name in `group`, or `None` when the declaration does not accept it.
///
/// Names are matched exactly: an array is named `name[]`, and a nested group's field `name[field]`, so `items[0]` does not name an element of `items`, and a scalar name does not match an array.
fn resolve(mut group: &GroupDeclaration, components: &[&str]) -> Option<Vec<usize>> {
    let (mut fields, mut rest) = (Vec::new(), components);
    loop {
        let (name, tail) = rest.split_first()?;
        let index = group.position(name)?;
        let field = &group.fields[index];
        fields.push(index);
        rest = tail;
        if field.container.count_limit().is_some() {
            let ([""], tail) = rest.split_at_checked(1)? else {
                return None;
            };
            rest = tail;
        }
        match field.kind {
            ItemKind::Group(inner) => group = inner,
            ItemKind::Scalar(_) | ItemKind::Upload => return rest.is_empty().then_some(fields),
        }
    }
}

/// Whether an upload whose part declared `media_type` is one `accept` allows. An empty `accept` allows any upload; otherwise a part without a `Content-Type` does not match.
fn accepts(accept: &[&'static str], media_type: Option<&MediaType>) -> bool {
    if accept.is_empty() {
        return true;
    }
    // Open question in design/parsing.md: whether to guess a media type from the filename's extension, as `protocol-content` does when a client declares none or `application/octet-stream`, is undecided until it is checked against real-world uploads. Until then only the declared media type is checked, and a filename never makes an upload acceptable.
    let Some(media_type) = media_type else {
        return false;
    };
    // SECURITY: `MediaRange::from_static` panics on a range that is not lowercase tokens, which would let any upload for the field panic the parser. It cannot here: the derive reads each range with `MediaRange::parse` from the same `base-web-media-type` and declares its lowercase essence, without parameters, which `from_static` accepts, and declarations are only written by the derive, as design/scope.md requires.
    let accept: MediaRangeSet = accept
        .iter()
        .map(|range| MediaRange::from_static(range))
        .collect();
    accept.contains(media_type)
}

/// A field after its value was checked, as the declared struct is built from it.
pub enum CheckedSlot<H> {
    /// An optional field that did not arrive.
    Absent,
    Single(CheckedValue<H>),
    Array(Vec<CheckedValue<H>>),
}

/// An item after its value was checked.
pub enum CheckedValue<H> {
    Scalar(Box<dyn Any>),
    Upload(Upload<H>),
    Group(Vec<CheckedSlot<H>>),
    /// An item with a violation.
    Failed,
}

/// The checked fields of a group, in declaration order.
pub struct GroupFields<H>(std::vec::IntoIter<CheckedSlot<H>>);

impl<H> GroupFields<H> {
    pub(crate) fn new(slots: Vec<CheckedSlot<H>>) -> Self {
        Self(slots.into_iter())
    }

    pub(crate) fn next_slot(&mut self) -> CheckedSlot<H> {
        self.0.next().expect("a group has a slot for each field")
    }
}

/// Convert each value of a group, and record a violation for each field that cannot be accepted or is required and absent, in declaration order.
fn check_group<H>(
    declaration: &GroupDeclaration,
    group: GroupValue<H>,
    path: &mut FieldPath,
    violations: &mut Vec<Violation>,
) -> Vec<CheckedSlot<H>> {
    let fields = declaration.fields.iter().zip(group.slots);
    fields
        .map(|(field, slot)| {
            path.push(FieldPathSegment::Name(field.name));
            let checked = match slot {
                // An array without elements arrived only as parts with no file chosen, which hold nothing.
                Slot::Array(items) if items.is_empty() => absent(field, path, violations),
                Slot::Empty | Slot::NoFile => absent(field, path, violations),
                Slot::Single(item) => {
                    CheckedSlot::Single(check_item(field, item, path, violations))
                }
                Slot::Array(items) => CheckedSlot::Array(
                    items
                        .into_iter()
                        .enumerate()
                        .map(|(index, item)| {
                            path.push(FieldPathSegment::Index(index));
                            let checked = check_item(field, item, path, violations);
                            path.pop();
                            checked
                        })
                        .collect(),
                ),
            };
            path.pop();
            checked
        })
        .collect()
}

/// A field that holds nothing: a violation when it is required.
fn absent<H>(
    field: &FieldDeclaration,
    path: &FieldPath,
    violations: &mut Vec<Violation>,
) -> CheckedSlot<H> {
    use crate::declaration::Container;
    if matches!(field.container, Container::Required | Container::Array(_)) {
        violations.push(Violation::new(path.clone(), ViolationCode::Required));
    }
    CheckedSlot::Absent
}

fn check_item<H>(
    field: &FieldDeclaration,
    item: ItemValue<H>,
    path: &mut FieldPath,
    violations: &mut Vec<Violation>,
) -> CheckedValue<H> {
    let code = match (item, field.kind) {
        (ItemValue::Scalar(bytes, charset), ItemKind::Scalar(convert)) => {
            match convert(bytes, charset) {
                Some(value) => return CheckedValue::Scalar(value),
                None => ViolationCode::InvalidType,
            }
        }
        (ItemValue::Upload(upload), _) => return CheckedValue::Upload(upload),
        (ItemValue::Group(group), ItemKind::Group(declaration)) => {
            return CheckedValue::Group(check_group(declaration, group, path, violations));
        }
        (ItemValue::Invalid(code), _) => code,
        _ => unreachable!("every field ended, and holds a value of its declared kind"),
    };
    violations.push(Violation::new(path.clone(), code));
    CheckedValue::Failed
}

/// Take every upload's handle from a group's values.
fn release_group<H>(group: GroupValue<H>, handles: &mut Vec<H>) {
    let release = |item: ItemValue<H>, handles: &mut Vec<H>| match item {
        ItemValue::Upload(upload) => handles.push(upload.into_handle()),
        ItemValue::Group(group) => release_group(group, handles),
        ItemValue::Pending | ItemValue::Scalar(..) | ItemValue::Invalid(_) => {}
    };
    for slot in group.slots {
        match slot {
            Slot::Empty | Slot::NoFile => {}
            Slot::Single(item) => release(item, handles),
            Slot::Array(items) => items.into_iter().for_each(|item| release(item, handles)),
        }
    }
}

/// Take every upload's handle from a group's checked values.
fn release_checked<H>(slots: Vec<CheckedSlot<H>>, handles: &mut Vec<H>) {
    let release = |value: CheckedValue<H>, handles: &mut Vec<H>| match value {
        CheckedValue::Upload(upload) => handles.push(upload.into_handle()),
        CheckedValue::Group(slots) => release_checked(slots, handles),
        CheckedValue::Scalar(_) | CheckedValue::Failed => {}
    };
    for slot in slots {
        match slot {
            CheckedSlot::Absent => {}
            CheckedSlot::Single(value) => release(value, handles),
            CheckedSlot::Array(values) => {
                values.into_iter().for_each(|value| release(value, handles))
            }
        }
    }
}
