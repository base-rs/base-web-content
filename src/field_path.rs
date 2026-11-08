// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use std::fmt;

/// The path to a field: the declared names and array indices that lead to it.
///
/// It holds only names the declaration gives and indices the parser counted, never submitted text, so it is safe to log.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct FieldPath {
    segments: Vec<FieldPathSegment>,
}

/// One step of a [`FieldPath`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FieldPathSegment {
    /// A declared field, by the name a form submits.
    Name(&'static str),
    /// An element of an array, counting from zero in the order the elements arrived.
    Index(usize),
}

impl FieldPath {
    /// The segments, from the top level.
    pub fn segments(&self) -> &[FieldPathSegment] {
        &self.segments
    }

    pub(crate) fn push(&mut self, segment: FieldPathSegment) {
        self.segments.push(segment);
    }

    pub(crate) fn pop(&mut self) {
        self.segments.pop();
    }

    pub(crate) fn extend(&mut self, segments: impl IntoIterator<Item = FieldPathSegment>) {
        self.segments.extend(segments);
    }
}

/// Writes the path as a form names it, with indices in brackets, such as `user[address][city]` or `items[1][count]`.
impl fmt::Display for FieldPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut text = String::new();
        for (position, segment) in self.segments.iter().enumerate() {
            match (position, segment) {
                (0, FieldPathSegment::Name(name)) => text.push_str(name),
                (_, FieldPathSegment::Name(name)) => text.push_str(&format!("[{name}]")),
                (_, FieldPathSegment::Index(index)) => text.push_str(&format!("[{index}]")),
            }
        }
        formatter.write_str(&text)
    }
}
