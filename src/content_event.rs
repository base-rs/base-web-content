// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::UploadHead;
use std::fmt;

/// What one call to [`ContentParser::parse`](crate::ContentParser::parse) found, with the number of input bytes it consumed.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ContentParsed<'a> {
    /// The input bytes consumed. The next call's input must begin with the bytes after them.
    pub consumed: usize,
    /// What the consumed bytes held for the caller.
    pub event: ContentEvent<'a>,
}

/// One step through a body, for the caller to act on. It is exhaustive, so that a caller can match it without a wildcard; adding an event is a breaking change.
///
/// Fields other than uploads are buffered by the parser and reported only when parsing finishes. An accepted upload produces `Upload`, any number of `UploadContent`, and `UploadEnd`, after which the caller must give the parser a handle for what it stored with [`ContentParser::store`](crate::ContentParser::store) before parsing on. A body ends with `End`.
#[derive(Clone, PartialEq, Eq)]
pub enum ContentEvent<'a> {
    /// An upload a declared field accepts begins. The caller should prepare storage for its content.
    Upload(UploadHead),
    /// Some of the current upload's content, borrowed from the input, for the caller to store. An upload's content may arrive in any number of pieces.
    UploadContent(&'a [u8]),
    /// The current upload's content is complete. The caller must give its handle with [`ContentParser::store`](crate::ContentParser::store).
    UploadEnd,
    /// The body is complete. Any input after it, a multipart body's epilogue, must still be given to later calls, which consume it within the epilogue limit and the body's maximum and report `End` again, or to [`ContentParser::parse_end`](crate::ContentParser::parse_end).
    End,
    /// The input ends before the next event: call again with more input, beginning with the bytes not yet consumed, or finish when the body has ended.
    Incomplete,
}

/// Shows the length of content rather than its bytes, since an upload's content must not reach logs.
impl fmt::Debug for ContentEvent<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Upload(head) => formatter.debug_tuple("Upload").field(head).finish(),
            Self::UploadContent(content) => {
                write!(formatter, "UploadContent(<{} bytes>)", content.len())
            }
            Self::UploadEnd => formatter.write_str("UploadEnd"),
            Self::End => formatter.write_str("End"),
            Self::Incomplete => formatter.write_str("Incomplete"),
        }
    }
}
