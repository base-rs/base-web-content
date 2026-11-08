// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! The declarations `#[derive(Content)]` generates, which the parser follows. They are not public API: applications declare content only through the derive.

use crate::ContentFormat;
use crate::field_value::Charset;
use std::any::Any;

/// Converts a scalar field's buffered bytes, with the character set its part declared, into its value, or `None` when they do not convert.
pub type Convert = fn(Vec<u8>, Charset) -> Option<Box<dyn Any>>;

/// What one form field or array element holds.
#[derive(Clone, Copy)]
pub enum ItemKind {
    /// Text or bytes, buffered and converted when parsing finishes.
    Scalar(Convert),
    /// A file, streamed to the caller.
    Upload,
    /// A nested group of fields.
    Group(&'static GroupDeclaration),
}

/// Whether a field must be present, may be omitted, or is an array, with the array's count limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Container {
    Required,
    Optional,
    Array(usize),
    OptionalArray(usize),
}

impl Container {
    /// The count limit of an array, or `None` for a single item.
    pub const fn count_limit(self) -> Option<usize> {
        match self {
            Self::Array(limit) | Self::OptionalArray(limit) => Some(limit),
            Self::Required | Self::Optional => None,
        }
    }
}

/// One field of a group, under the name a form submits.
pub struct FieldDeclaration {
    pub name: &'static str,
    pub kind: ItemKind,
    pub container: Container,
    /// The declared size limit of one item, or `None` for the default of its kind.
    pub size_limit: Option<usize>,
    /// The media ranges an upload accepts, lowercase and without parameters; empty to accept any.
    pub accept: &'static [&'static str],
}

impl FieldDeclaration {
    /// The size limit of one item's content.
    pub(crate) fn size_limit(&self) -> usize {
        match (self.size_limit, self.kind) {
            (Some(limit), _) => limit,
            (None, ItemKind::Upload) => UPLOAD_SIZE_LIMIT,
            (None, _) => FIELD_SIZE_LIMIT,
        }
    }

    /// The number of items the field can hold: one, or its array's count limit.
    fn multiplicity(&self) -> usize {
        self.container.count_limit().unwrap_or(1)
    }
}

/// The fields of a struct deriving `Content`, in declaration order.
pub struct GroupDeclaration {
    pub fields: &'static [FieldDeclaration],
}

impl GroupDeclaration {
    /// The index of the field with this submitted name.
    pub(crate) fn position(&self, name: &str) -> Option<usize> {
        self.fields.iter().position(|field| field.name == name)
    }
}

/// The default size limit of a text, bytes, or converted scalar field: 64 KiB.
pub const FIELD_SIZE_LIMIT: usize = 64 * 1024;
/// The default size limit of an upload: 128 MiB.
pub const UPLOAD_SIZE_LIMIT: usize = 128 * 1024 * 1024;
/// The default count limit of an array.
pub const COUNT_LIMIT: usize = 128;

/// The limits of a declaration that apply to the whole body, rather than to one field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// The components of one name.
    pub depth_limit: usize,
    /// The content of every field and upload together.
    pub content_size_limit: usize,
    /// The parts of a `multipart/form-data` body.
    pub part_count_limit: usize,
    /// One part's header section, including padding after its boundary.
    pub header_size_limit: usize,
    /// The fields in one part's header section.
    pub header_count_limit: usize,
    /// The bytes before the first boundary.
    pub preamble_size_limit: usize,
    /// The bytes after the closing boundary line.
    pub epilogue_size_limit: usize,
    /// The pairs of an `application/x-www-form-urlencoded` body.
    pub pair_count_limit: usize,
    /// An `application/x-www-form-urlencoded` body.
    pub url_encoded_size_limit: usize,
}

impl Limits {
    pub const DEFAULT: Self = Self {
        depth_limit: 8,
        content_size_limit: 256 * 1024 * 1024,
        part_count_limit: 128,
        header_size_limit: 8 * 1024,
        header_count_limit: 64,
        preamble_size_limit: 64 * 1024,
        epilogue_size_limit: 1024,
        pair_count_limit: 1024,
        url_encoded_size_limit: 2 * 1024 * 1024,
    };
}

/// The formats a declaration accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Formats {
    pub multipart_form_data: bool,
    pub url_encoded: bool,
}

impl Formats {
    pub const ALL: Self = Self {
        multipart_form_data: true,
        url_encoded: true,
    };

    pub(crate) fn accepts(self, format: ContentFormat) -> bool {
        match format {
            ContentFormat::MultipartFormData => self.multipart_form_data,
            ContentFormat::UrlEncoded => self.url_encoded,
        }
    }
}

/// A struct's declaration, as the top level of a body.
pub struct ContentDeclaration {
    pub group: &'static GroupDeclaration,
    pub limits: Limits,
    pub formats: Formats,
}

/// The most components a name in this group can have: its own field's name, `[]` for an array, and the names within a nested group.
pub const fn depth(group: &GroupDeclaration) -> usize {
    let mut deepest = 0;
    let mut index = 0;
    while index < group.fields.len() {
        let field = &group.fields[index];
        let mut depth = 1;
        if field.container.count_limit().is_some() {
            depth += 1;
        }
        if let ItemKind::Group(group) = field.kind {
            depth += self::depth(group);
        }
        if depth > deepest {
            deepest = depth;
        }
        index += 1;
    }
    deepest
}

/// The longest boundary line before a part: CRLF, `--`, a boundary of at most 70 characters, as RFC 2046 allows, and CRLF.
const BOUNDARY_LINE_SIZE: usize = 2 + 2 + 70 + 2;
/// The longest closing boundary line, without transport padding: CRLF, `--`, the boundary, `--`, and CRLF.
const CLOSING_LINE_SIZE: usize = 2 + 2 + 70 + 2 + 2;
/// The empty line that ends a part's header section.
const EMPTY_LINE_SIZE: usize = 2;

impl ContentDeclaration {
    /// The largest body in `format` the declaration could need, or `None` when it does not accept the format.
    pub(crate) fn body_size_limit(&self, format: ContentFormat) -> Option<usize> {
        if !self.formats.accepts(format) {
            return None;
        }
        Some(match format {
            ContentFormat::MultipartFormData => self.multipart_body_size_limit(),
            ContentFormat::UrlEncoded => self.url_encoded_body_size_limit(),
        })
    }

    fn multipart_body_size_limit(&self) -> usize {
        let limits = &self.limits;
        let parts = limits.part_count_limit.min(item_count(self.group));
        let part = BOUNDARY_LINE_SIZE
            .saturating_add(limits.header_size_limit)
            .saturating_add(EMPTY_LINE_SIZE);
        let content = limits.content_size_limit.min(content_size(self.group));
        limits
            .preamble_size_limit
            .saturating_add(parts.saturating_mul(part))
            .saturating_add(content)
            .saturating_add(CLOSING_LINE_SIZE)
            .saturating_add(limits.epilogue_size_limit)
    }

    fn url_encoded_body_size_limit(&self) -> usize {
        let mut pairs = Vec::new();
        url_encoded_pairs(self.group, 0, 1, &mut pairs);
        // The largest pairs, up to the pair count limit, since that is the most a body could hold.
        pairs.sort_unstable_by_key(|&(pair, _)| std::cmp::Reverse(pair));
        let (mut remaining, mut size) = (self.limits.pair_count_limit, 0usize);
        for (pair, count) in pairs {
            let count = count.min(remaining);
            size = size.saturating_add(pair.saturating_mul(count));
            remaining -= count;
        }
        size.min(self.limits.url_encoded_size_limit)
    }
}

/// The scalars and uploads a group could hold: one for each, and the count limit for each array, within nested groups as well.
fn item_count(group: &GroupDeclaration) -> usize {
    group.fields.iter().fold(0, |count, field| {
        let items = match field.kind {
            ItemKind::Group(group) => item_count(group),
            ItemKind::Scalar(_) | ItemKind::Upload => 1,
        };
        count.saturating_add(field.multiplicity().saturating_mul(items))
    })
}

/// The sum of the content limits of every item a group could hold.
fn content_size(group: &GroupDeclaration) -> usize {
    group.fields.iter().fold(0, |size, field| {
        let item = match field.kind {
            ItemKind::Group(group) => content_size(group),
            ItemKind::Scalar(_) | ItemKind::Upload => field.size_limit(),
        };
        size.saturating_add(field.multiplicity().saturating_mul(item))
    })
}

/// The largest URL-encoded pair for each scalar or upload in `group`, with how many of them the group could hold: three times the bytes of the longest name, and of the content limit, since percent-encoding at most triples a byte, a `=`, and a `&`.
///
/// `prefix` is the length of the name that leads to the group, or 0 at the top level, and `multiplicity` the number of groups it could hold.
fn url_encoded_pairs(
    group: &GroupDeclaration,
    prefix: usize,
    multiplicity: usize,
    pairs: &mut Vec<(usize, usize)>,
) {
    for field in group.fields {
        // A nested field's name is a `[key]` component, and an array adds `[]`.
        let brackets = if prefix == 0 { 0 } else { 2 };
        let array = if field.container.count_limit().is_some() {
            2
        } else {
            0
        };
        let name = prefix + field.name.len() + brackets + array;
        let multiplicity = multiplicity.saturating_mul(field.multiplicity());
        match field.kind {
            ItemKind::Group(group) => url_encoded_pairs(group, name, multiplicity, pairs),
            ItemKind::Scalar(_) | ItemKind::Upload => {
                let pair = name
                    .saturating_mul(3)
                    .saturating_add(field.size_limit().saturating_mul(3))
                    .saturating_add(2);
                pairs.push((pair, multiplicity));
            }
        }
    }
}
