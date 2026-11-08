// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Declarative, limited parsing of web request content, such as forms, without I/O.
//!
//! Applications declare the content they accept with [`#[derive(Content)]`](Content), with limits on every field, array, and upload, so that a body is never parsed without knowing its limits. [`ContentParser`] is fed bytes as they arrive and refuses whatever the declaration does not accept as early as the bytes allow: a name the declaration does not accept before its content is read, and any limit as soon as it is exceeded, with a fatal [`ContentError`]. Uploads are reported as events for the caller to store, and the other fields are converted strictly, with every [`Violation`] returned together when parsing finishes. The crate does no I/O.
//!
//! `multipart/form-data` is parsed with `base-web-multipart`, and `application/x-www-form-urlencoded` with `base-web-url`, whose bracket paths name the fields of both. The [Getting Started](https://github.com/base-rs/base-web-content/blob/main/context/getting-started.md) guide covers declarations, limits, uploads, and errors.

// The derive's generated code names this crate, so that it can be used within the crate's own tests.
#[cfg(test)]
extern crate self as base_web_content;

mod accepted;
mod content;
mod content_error;
mod content_event;
mod content_format;
mod content_parser;
mod declaration;
mod field_path;
mod field_value;
mod form;
mod rejected;
mod upload;
mod violation;

#[doc(hidden)]
#[path = "private.rs"]
pub mod __private;

pub use accepted::Accepted;
pub use base_web_content_macros::{Content, FieldValue};
pub use base_web_multipart::ParameterRules;
pub use content::{Content, ContentHandles};
pub use content_error::{ContentError, ContentErrorKind};
pub use content_event::{ContentEvent, ContentParsed};
pub use content_format::ContentFormat;
pub use content_parser::ContentParser;
pub use field_path::{FieldPath, FieldPathSegment};
pub use field_value::{FieldValue, FieldValueError};
pub use rejected::{Rejected, Rejection};
pub use upload::{Upload, UploadHead};
pub use violation::{Violation, ViolationCode};

#[cfg(doctest)]
#[doc = include_str!("../readme.md")]
struct Readme;

#[cfg(doctest)]
#[doc = include_str!("../context/getting-started.md")]
struct GettingStartedGuide;
