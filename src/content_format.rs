// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_web_media_type::MediaType;

/// A format of request content this crate parses, chosen by the request's media type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ContentFormat {
    /// `multipart/form-data`, as RFC 7578 defines it.
    MultipartFormData,
    /// `application/x-www-form-urlencoded`, as the WHATWG URL Standard defines it.
    UrlEncoded,
}

impl ContentFormat {
    /// The format of content with this media type, by its essence, or `None` for a type this crate does not parse.
    ///
    /// ```
    /// use base_web_content::ContentFormat;
    /// use base_web_media_type::MediaType;
    ///
    /// let media_type = MediaType::parse("multipart/form-data; boundary=x")?;
    /// assert_eq!(ContentFormat::from_media_type(&media_type), Some(ContentFormat::MultipartFormData));
    /// assert_eq!(ContentFormat::from_media_type(&MediaType::APPLICATION_JSON), None);
    /// # Ok::<(), base_web_media_type::MediaTypeError>(())
    /// ```
    pub fn from_media_type(media_type: &MediaType) -> Option<Self> {
        [Self::MultipartFormData, Self::UrlEncoded]
            .into_iter()
            .find(|format| format.media_type().essence() == media_type.essence())
    }

    /// The format's media type, without parameters.
    pub fn media_type(self) -> MediaType {
        match self {
            Self::MultipartFormData => MediaType::MULTIPART_FORM_DATA,
            Self::UrlEncoded => MediaType::APPLICATION_X_WWW_FORM_URLENCODED,
        }
    }
}
