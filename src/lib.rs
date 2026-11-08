// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Declarative, limited parsing of web request content, such as forms, without I/O.
//!
//! Applications declare the content they accept, with limits on every field and upload, so that a body is never parsed without knowing its limits. The crate is fed bytes and does no I/O. The [Getting Started](https://github.com/base-rs/base-web-content/blob/main/context/getting-started.md) guide describes the crate's current state.

// The derive's generated code names this crate, so that it can be used within the crate's own tests.
#[cfg(test)]
extern crate self as base_web_content;

mod accepted;
mod content;
mod content_error;
mod content_format;
mod declaration;
mod field_path;
mod field_value;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the multipart parser, which drives the form, comes in the next change"
    )
)]
mod form;
mod rejected;
mod upload;
mod violation;

#[doc(hidden)]
#[path = "private.rs"]
pub mod __private;

pub use accepted::Accepted;
pub use base_web_content_macros::{Content, FieldValue};
pub use content::Content;
pub use content_error::{ContentError, ContentErrorKind};
pub use content_format::ContentFormat;
pub use field_path::{FieldPath, FieldPathSegment};
pub use field_value::{FieldValue, FieldValueError};
pub use rejected::{Rejected, Rejection};
pub use upload::{Upload, UploadHead};
pub use violation::{Violation, ViolationCode};
