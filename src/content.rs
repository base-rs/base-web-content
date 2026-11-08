// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::__private::{Declaration, Item};
use crate::ContentFormat;

/// Content an application declares that it accepts, with limits on everything in it. Derive it with `#[derive(Content)]` on a struct with named fields; it must not be implemented by hand, since what the derive implements is not public API.
///
/// Each field declares one form field or nested group, named by its identifier unless `rename` gives the name a form submits:
///
/// | Field type | Submitted as |
/// | --- | --- |
/// | `String` | Text, which must be valid UTF-8. |
/// | `Vec<u8>` | The submitted bytes, unchanged. |
/// | An integer, `f32`, `f64`, or `bool` | Text in the type's strict syntax, such as `-12`, `1.5e3`, or `true`. |
/// | A [`FieldValue`](crate::FieldValue), such as an enumeration with `#[derive(FieldValue)]` | Text converted by its code. |
/// | [`Upload<H>`](crate::Upload) | A file, with the caller's storage handle `H`. |
/// | A struct deriving `Content` | A nested group, whose fields are submitted as `name[field]`. |
/// | `Option<T>` | A field the content may omit. |
/// | `Vec<T>`, other than `Vec<u8>` | An array, submitted as `name[]`, or `name[][field]` for an array of groups. |
///
/// Every field without `Option` is required. Names are matched exactly, and a name the declaration does not accept is a fatal error before its content is read.
///
/// Each field and the struct take `#[content(...)]` attributes, whose limits are constant `usize` expressions:
///
/// | Attribute | On | Default |
/// | --- | --- | --- |
/// | `rename = "name"` | A field | The field's identifier |
/// | `size_limit = N` | A scalar or upload, or each element of an array | 64 KiB, or 128 MiB for an upload |
/// | `count_limit = N` | An array | 128 |
/// | `accept("image/png", "image/*")`, matched against the media type the upload's part declares, so that a part without one is refused even by `*/*` | An upload | Any upload |
/// | `formats("multipart/form-data", "application/x-www-form-urlencoded")` | The struct | Both |
/// | `depth_limit = N`, the components of a name | The struct | 8 |
/// | `content_size_limit = N`, the content of every field together | The struct | 256 MiB |
/// | `part_count_limit = N` | The struct | 128 |
/// | `header_size_limit = N`, one part's header section | The struct | 8 KiB |
/// | `header_count_limit = N`, the fields of one part's header section | The struct | 64 |
/// | `preamble_size_limit = N` | The struct | 64 KiB |
/// | `epilogue_size_limit = N` | The struct | 1 KiB |
/// | `pair_count_limit = N`, the pairs of a URL-encoded body | The struct | 1,024 |
/// | `url_encoded_size_limit = N` | The struct | 2 MiB |
///
/// A struct's limits apply when it is the top level of a body, not when it is nested in another. Every upload in a declaration, nested groups included, must have the same handle type `H`, which [`ContentHandles`] names.
///
/// ```
/// use base_web_content::{Content, ContentFormat, FieldValue, Upload};
///
/// #[derive(FieldValue)]
/// enum Plan {
///     #[content(rename = "free")]
///     Free,
///     #[content(rename = "pro")]
///     Pro,
/// }
///
/// #[derive(Content)]
/// struct Address {
///     city: String,
///     postcode: Option<String>,
/// }
///
/// #[derive(Content)]
/// #[content(formats("multipart/form-data"))]
/// struct Signup {
///     #[content(size_limit = 256)]
///     email: String,
///     plan: Plan,
///     address: Option<Address>,
///     #[content(count_limit = 8)]
///     interests: Vec<String>,
///     #[content(accept("image/png", "image/jpeg"), size_limit = 4 * 1024 * 1024)]
///     avatar: Option<Upload<std::path::PathBuf>>,
/// }
///
/// assert!(Signup::body_size_limit(ContentFormat::MultipartFormData).is_some());
/// assert_eq!(Signup::body_size_limit(ContentFormat::UrlEncoded), None);
/// ```
pub trait Content: Declaration {
    /// The largest body in `format` the declaration could need, computed from its fields and limits, or `None` when it does not accept the format.
    ///
    /// A server can refuse a request whose `Content-Length` exceeds it with 413 Content Too Large before reading the body. The parser enforces it too, as a running total, whatever the request declared.
    fn body_size_limit(format: ContentFormat) -> Option<usize> {
        Self::DECLARATION.body_size_limit(format)
    }
}

/// A declaration whose uploads hold handles of type `H`: any declaration without uploads, for every `H`, and one whose uploads are `Upload<H>`. It is implemented for every such declaration, so that generic code can name what [`ContentParser`](crate::ContentParser) parses.
///
/// ```
/// use base_web_content::{ContentError, ContentHandles, ContentParser};
/// use base_web_media_type::MediaType;
///
/// fn parser_for<T: ContentHandles<std::path::PathBuf>>(media_type: &MediaType) -> Result<ContentParser<T, std::path::PathBuf>, ContentError> {
///     ContentParser::new(Some(media_type))
/// }
/// ```
pub trait ContentHandles<H>: Content + Item<H> {}

impl<T: Content + Item<H>, H> ContentHandles<H> for T {}
