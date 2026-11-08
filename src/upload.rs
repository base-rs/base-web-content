// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::FieldPath;
use base_web_media_type::MediaType;
use std::fmt;

/// A file a form submitted, with the caller's handle for where its content was stored.
///
/// The parser does not keep an upload's content: it reports the content to the caller as it arrives, and the caller stores it, such as in a temporary file, and gives the parser a handle of type `H` for it. The filename and media type are what the client declared, and must not be trusted: a filename can contain path separators or be empty, and a media type need not describe the content.
pub struct Upload<H> {
    filename: String,
    media_type: Option<MediaType>,
    size: u64,
    handle: H,
}

impl<H> Upload<H> {
    /// An upload as the parser would give it, such as for an application's own tests of what it does with its declared struct.
    pub fn new(
        untrusted_filename: impl Into<String>,
        untrusted_media_type: Option<MediaType>,
        size: u64,
        handle: H,
    ) -> Self {
        Self {
            filename: untrusted_filename.into(),
            media_type: untrusted_media_type,
            size,
            handle,
        }
    }

    pub(crate) fn from_head(head: UploadHead, size: u64, handle: H) -> Self {
        Self::new(head.filename, head.media_type, size, handle)
    }

    /// The filename the client declared, which may be empty, as browsers send for a file input left empty. It must not be used as a local path without checking it.
    pub fn untrusted_filename(&self) -> &str {
        &self.filename
    }

    /// The media type the client declared for the content, if any, which need not describe the content.
    pub fn untrusted_media_type(&self) -> Option<&MediaType> {
        self.media_type.as_ref()
    }

    /// The bytes of content the parser reported.
    pub fn size(&self) -> u64 {
        self.size
    }

    /// The caller's handle for the stored content.
    pub fn handle(&self) -> &H {
        &self.handle
    }

    /// The handle, taken from the upload.
    pub fn into_handle(self) -> H {
        self.handle
    }
}

/// Shows the size only, since the filename and media type are submitted content, and the handle may name them.
impl<H> fmt::Debug for Upload<H> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Upload")
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

/// An upload that begins: the field it is for, and what the client declared about it, which must not be trusted.
#[derive(Clone, PartialEq, Eq)]
pub struct UploadHead {
    path: FieldPath,
    filename: String,
    media_type: Option<MediaType>,
}

impl UploadHead {
    pub(crate) fn new(path: FieldPath, filename: String, media_type: Option<MediaType>) -> Self {
        Self {
            path,
            filename,
            media_type,
        }
    }

    /// The declared field the upload is for, with its index when the field is an array.
    pub fn path(&self) -> &FieldPath {
        &self.path
    }

    /// The filename the client declared, which may be empty. It must not be used as a local path without checking it.
    pub fn untrusted_filename(&self) -> &str {
        &self.filename
    }

    /// The media type the client declared, which the field's `accept` allowed, but need not describe the content.
    pub fn untrusted_media_type(&self) -> Option<&MediaType> {
        self.media_type.as_ref()
    }
}

/// Shows the path and the filename's length, since the filename and media type are submitted content.
impl fmt::Debug for UploadHead {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UploadHead")
            .field("path", &format_args!("{}", self.path))
            .field("filename", &format_args!("<{} bytes>", self.filename.len()))
            .finish_non_exhaustive()
    }
}
