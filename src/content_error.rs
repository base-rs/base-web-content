// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_web_multipart::{MultipartError, MultipartErrorKind};
use base_web_url::{FormError, FormErrorKind};
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
    /// An `application/x-www-form-urlencoded` media type declares a `charset` other than `UTF-8` or `US-ASCII`. 415 Unsupported Media Type.
    UnsupportedCharset,
    /// The body is longer than the largest the declaration could need, [`Content::body_size_limit`](crate::Content::body_size_limit). 413 Content Too Large.
    BodySizeLimit,
    /// The bytes before a `multipart/form-data` body's first boundary exceed the preamble limit. 413 Content Too Large.
    PreambleSizeLimit,
    /// The bytes after a `multipart/form-data` body's closing boundary line exceed the epilogue limit. 413 Content Too Large.
    EpilogueSizeLimit,
    /// A part's header section, with any padding after its boundary, exceeds the header section limit. 413 Content Too Large.
    HeaderSizeLimit,
    /// A part's header section has more fields than the header count limit. 413 Content Too Large.
    HeaderCountLimit,
    /// The body has more parts than the part count limit. 413 Content Too Large.
    PartCountLimit,
    /// An `application/x-www-form-urlencoded` body has more pairs than the pair count limit. 413 Content Too Large.
    PairCountLimit,
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
    /// A `multipart/form-data` media type has no `boundary` parameter, or one RFC 2046 does not allow. 400 Bad Request.
    InvalidBoundary,
    /// A part's `Content-Disposition` is missing, is not `form-data`, or has no `name`. 400 Bad Request.
    MissingName,
    /// A part's header fields cannot be read: a header line is malformed or folded, a `Content-Disposition` or `Content-Type` is invalid or repeated, its parameters break the [`ParameterRules`](crate::ParameterRules), or its `Content-Transfer-Encoding` is not an identity encoding. 400 Bad Request.
    InvalidPartHeader,
    /// A part for a declared upload has an empty `filename`, which browsers send for a file input with no file chosen, yet has content. 400 Bad Request.
    UnnamedFile,
    /// The content ended before it was complete: a multipart body before its closing boundary, or URL-encoded content whose end was not given to [`ContentParser::parse_end`](crate::ContentParser::parse_end). 400 Bad Request.
    UnexpectedEnd,
    /// The multipart framing is malformed, as `base-web-multipart` reports it, such as a boundary line that does not end with a line break, or a boundary after a bare CR or LF. 400 Bad Request.
    MalformedMultipart,
    /// URL-encoded content is malformed, as `base-web-url` reports it: a `%` not followed by two hexadecimal digits, or decoded text that is not UTF-8. 400 Bad Request.
    MalformedUrlEncoded,
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
            Self::MissingMediaType
            | Self::UnsupportedMediaType
            | Self::UnacceptedFormat
            | Self::UnsupportedCharset => 415,
            Self::BodySizeLimit
            | Self::PreambleSizeLimit
            | Self::EpilogueSizeLimit
            | Self::HeaderSizeLimit
            | Self::HeaderCountLimit
            | Self::PartCountLimit
            | Self::PairCountLimit
            | Self::DepthLimit
            | Self::CountLimit
            | Self::FieldSizeLimit
            | Self::ContentSizeLimit => 413,
            Self::UnknownName
            | Self::InvalidBoundary
            | Self::MissingName
            | Self::InvalidPartHeader
            | Self::UnnamedFile
            | Self::UnexpectedEnd
            | Self::MalformedMultipart
            | Self::MalformedUrlEncoded => 400,
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::MissingMediaType => "content has no media type",
            Self::UnsupportedMediaType => "content has an unsupported media type",
            Self::UnacceptedFormat => "content is in a format the declaration does not accept",
            Self::UnsupportedCharset => "URL-encoded content has an unsupported character set",
            Self::PairCountLimit => "URL-encoded content exceeds its pair count limit",
            Self::UnexpectedEnd => "content ends before it is complete",
            Self::BodySizeLimit => "content exceeds the largest body its declaration accepts",
            Self::PreambleSizeLimit => "multipart preamble exceeds its size limit",
            Self::EpilogueSizeLimit => "multipart epilogue exceeds its size limit",
            Self::HeaderSizeLimit => "multipart part header section exceeds its size limit",
            Self::HeaderCountLimit => "multipart part header section exceeds its field count limit",
            Self::PartCountLimit => "multipart body exceeds its part count limit",
            Self::DepthLimit => "a name is deeper than its depth limit",
            Self::CountLimit => "an array has more elements than its count limit",
            Self::FieldSizeLimit => "a field exceeds its size limit",
            Self::ContentSizeLimit => "content exceeds its total size limit",
            Self::UnknownName => "a name is not declared",
            Self::InvalidBoundary => "multipart media type has no valid boundary",
            Self::MissingName => "multipart part has no form-data name",
            Self::InvalidPartHeader => "multipart part has invalid header fields",
            Self::UnnamedFile => "multipart part with an empty filename has content",
            Self::MalformedMultipart => "multipart body is malformed",
            Self::MalformedUrlEncoded => "URL-encoded content is malformed",
        }
    }
}

impl ContentError {
    pub(crate) fn new(kind: ContentErrorKind, offset: u64) -> Self {
        Self { kind, offset }
    }

    /// What went wrong.
    pub fn kind(&self) -> ContentErrorKind {
        self.kind
    }

    /// The offset in the body, in bytes, where the parser found the problem: 0 for a problem with the media type; the start of a part's header section for a problem with the section, its size, its fields, or the part's name, which are decided when the section is complete; the start of a URL-encoded name for a problem with the name, or the first byte beyond the longest it could be; and otherwise the first byte beyond a limit, or the first byte that is malformed.
    pub fn offset(&self) -> u64 {
        self.offset
    }

    /// The HTTP status a server should respond with, from the kind.
    pub fn status(&self) -> u16 {
        self.kind.status()
    }
}

/// A `base-web-url` error, with the same offset: its pair count limit is this crate's, and a name longer than the longest the declaration accepts could make it is a name the declaration does not accept. Its other limits are wider than this crate's, which are reached first, and the rest are malformed content.
pub(crate) fn from_form_error(error: FormError) -> ContentError {
    let kind = match error.kind() {
        FormErrorKind::PairCountLimit => ContentErrorKind::PairCountLimit,
        FormErrorKind::NameLengthLimit => ContentErrorKind::UnknownName,
        // The size and value length limits are this crate's body and field limits, which are reached first, and paths are read by the form, so only malformed content remains.
        _ => ContentErrorKind::MalformedUrlEncoded,
    };
    ContentError::new(kind, error.offset())
}

/// A `base-web-multipart` error, with the same offset: its limits are this crate's, a malformed header line is a part header that cannot be read, and anything else is malformed framing.
pub(crate) fn from_multipart_error(error: MultipartError) -> ContentError {
    let kind = match error.kind() {
        MultipartErrorKind::PreambleSizeLimit => ContentErrorKind::PreambleSizeLimit,
        MultipartErrorKind::HeaderSizeLimit => ContentErrorKind::HeaderSizeLimit,
        MultipartErrorKind::HeaderCountLimit => ContentErrorKind::HeaderCountLimit,
        MultipartErrorKind::PartCountLimit => ContentErrorKind::PartCountLimit,
        MultipartErrorKind::InvalidField | MultipartErrorKind::FoldedField => {
            ContentErrorKind::InvalidPartHeader
        }
        MultipartErrorKind::UnexpectedEnd => ContentErrorKind::UnexpectedEnd,
        _ => ContentErrorKind::MalformedMultipart,
    };
    ContentError::new(kind, error.offset())
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
