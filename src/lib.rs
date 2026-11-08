// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Declarative, limited parsing of web request content, such as forms, without I/O.
//!
//! Applications declare the content they accept, with limits on every field and upload, so that a body is never parsed without knowing its limits. The crate is fed bytes and does no I/O. The [Getting Started](https://github.com/base-rs/base-web-content/blob/main/context/getting-started.md) guide describes the crate's current state.
