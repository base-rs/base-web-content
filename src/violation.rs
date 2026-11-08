// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::FieldPath;
use std::fmt;

/// A declared field that arrived but cannot be accepted, or a required one that did not arrive.
///
/// Violations do not stop parsing: every one in the body is collected and returned when it ends, in declaration order, so that an application can present them all. A violation never includes the submitted value, only the field's path and a code.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Violation {
    path: FieldPath,
    code: ViolationCode,
}

impl Violation {
    pub(crate) fn new(path: FieldPath, code: ViolationCode) -> Self {
        Self { path, code }
    }

    /// The field's path: the declared names and array indices that lead to it.
    pub fn path(&self) -> &FieldPath {
        &self.path
    }

    /// What is wrong with the field.
    pub fn code(&self) -> ViolationCode {
        self.code
    }
}

/// Writes the path and code, such as `items[1][count]: invalid_type`.
impl fmt::Display for Violation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.code)
    }
}

/// What is wrong with a field, as a [`Violation`] reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ViolationCode {
    /// A required field did not arrive: a scalar or upload was absent, an array had no elements, or a nested group received none of its fields.
    Required,
    /// The value does not convert to the field's type, is not valid UTF-8, has an unsupported character set, or is a file where text is declared or the reverse.
    InvalidType,
    /// The upload's declared media type is not one its field accepts.
    UnsupportedMediaType,
}

impl ViolationCode {
    /// The code as it is written, such as `invalid_type`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Required => "required",
            Self::InvalidType => "invalid_type",
            Self::UnsupportedMediaType => "unsupported_media_type",
        }
    }
}

impl fmt::Display for ViolationCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
