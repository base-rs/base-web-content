// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use std::fmt;

/// Why content could not be parsed at all, with the offset in the body where the parser found it.
///
/// A fatal error stops parsing, and the parser reports the same error for every later call. It never includes submitted content, field values, filenames, media types, or names the declaration does not accept: only its kind and offset. Each kind maps to one HTTP status, from [`Self::status`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ContentError {
    kind: ContentErrorKind,
    offset: u64,
}

/// The kind of [`ContentError`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ContentErrorKind {
    /// The request has no media type. 415 Unsupported Media Type.
    MissingMediaType,
    /// The request's media type is not a format this crate parses. 415 Unsupported Media Type.
    UnsupportedMediaType,
    /// The declaration does not accept the request's format. 415 Unsupported Media Type.
    UnacceptedFormat,
    /// A name has more components than the depth limit. 413 Content Too Large.
    DepthLimit,
    /// An array has more elements than its count limit. 413 Content Too Large.
    CountLimit,
    /// A field or upload has more content than its size limit. 413 Content Too Large.
    FieldSizeLimit,
    /// The fields and uploads together have more content than the total content limit. 413 Content Too Large.
    ContentSizeLimit,
    /// A name is not one the declaration accepts, including a name that is not `base`, `base[key]`, or `base[]`, repeated. 400 Bad Request.
    UnknownName,
}

impl ContentErrorKind {
    /// The HTTP status a server should respond with: 415, 413, or 400.
    ///
    /// ```
    /// use base_web_content::ContentErrorKind;
    ///
    /// assert_eq!(ContentErrorKind::FieldSizeLimit.status(), 413);
    /// ```
    pub fn status(self) -> u16 {
        match self {
            Self::MissingMediaType | Self::UnsupportedMediaType | Self::UnacceptedFormat => 415,
            Self::DepthLimit | Self::CountLimit | Self::FieldSizeLimit | Self::ContentSizeLimit => {
                413
            }
            Self::UnknownName => 400,
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::MissingMediaType => "content has no media type",
            Self::UnsupportedMediaType => "content has an unsupported media type",
            Self::UnacceptedFormat => "content is in a format the declaration does not accept",
            Self::DepthLimit => "a name is deeper than its depth limit",
            Self::CountLimit => "an array has more elements than its count limit",
            Self::FieldSizeLimit => "a field exceeds its size limit",
            Self::ContentSizeLimit => "content exceeds its total size limit",
            Self::UnknownName => "a name is not declared",
        }
    }
}

impl ContentError {
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the multipart parser, which reports errors, comes in the next change"
        )
    )]
    pub(crate) fn new(kind: ContentErrorKind, offset: u64) -> Self {
        Self { kind, offset }
    }

    /// What went wrong.
    pub fn kind(&self) -> ContentErrorKind {
        self.kind
    }

    /// The offset in the body, in bytes, where the parser found the problem: 0 for a problem with the media type.
    pub fn offset(&self) -> u64 {
        self.offset
    }

    /// The HTTP status a server should respond with, from the kind.
    pub fn status(&self) -> u16 {
        self.kind.status()
    }
}

impl fmt::Display for ContentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} at byte {}",
            self.kind.description(),
            self.offset
        )
    }
}

impl std::error::Error for ContentError {}
