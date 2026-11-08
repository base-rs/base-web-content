// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use super::Stepped;
use crate::content_error::from_form_error;
use crate::declaration::ContentDeclaration;
use crate::field_value::Charset;
use crate::form::{FieldSource, Form};
use crate::{ContentError, ContentErrorKind, ContentEvent};
use base_web_media_type::MediaType;
use base_web_url::{FormError, FormEvent, FormLimits, FormParser};

/// `application/x-www-form-urlencoded` content, read by `base-web-url`, whose pairs drive the form.
pub(super) struct UrlEncodedBody {
    parser: FormParser,
    /// The character set the media type declared, which each value is read in.
    charset: Charset,
}

impl UrlEncodedBody {
    pub(super) fn new(
        media_type: &MediaType,
        declaration: &ContentDeclaration,
    ) -> Result<Self, ContentErrorKind> {
        let charset = Charset::from_parameter(media_type.parameter("charset"));
        if charset == Charset::Unsupported {
            return Err(ContentErrorKind::UnsupportedCharset);
        }
        let limits = &declaration.limits;
        let parser = FormParser::with_limits(FormLimits {
            size_limit: limits.url_encoded_size_limit,
            pair_count_limit: limits.pair_count_limit,
            // Percent-encoding at most triples a name, so a longer one is not a name the declaration accepts, and is refused before it is read whole.
            name_length_limit: declaration.longest_name().saturating_mul(3),
            // Values are limited by their fields, as they arrive decoded.
            value_length_limit: limits.url_encoded_size_limit,
            depth_limit: limits.depth_limit,
        });
        Ok(Self { parser, charset })
    }

    pub(super) fn minimum_buffer_size(&self) -> usize {
        self.parser.minimum_buffer_size()
    }

    /// Parse until there is an event for the caller. `offset` is the offset of `input` in the content, and `end` whether the content ends with it.
    pub(super) fn parse<'a, H>(
        &mut self,
        form: &mut Form<H>,
        input: &'a [u8],
        offset: usize,
        end: bool,
    ) -> Result<Stepped<'a>, ContentError> {
        let mut consumed = 0;
        loop {
            let rest = &input[consumed..];
            let result = if end {
                self.parser.parse_end(rest)
            } else {
                self.parser.parse(rest)
            };
            let parsed = match result {
                Ok(parsed) => parsed,
                Err(error) => return Err(refuse(form, rest, offset + consumed, error)),
            };
            let start = consumed;
            consumed += parsed.consumed;
            match parsed.event {
                FormEvent::Name(name) => {
                    // A name without `=` has no value, which no field type accepts.
                    let source = if name.has_value() {
                        FieldSource::Value(self.charset)
                    } else {
                        FieldSource::Absent
                    };
                    form.begin(name.as_str(), source)
                        .map_err(|kind| ContentError::new(kind, name.offset()))?;
                }
                FormEvent::Value(value) => {
                    let (accepted, error) = form.content(value.as_bytes());
                    if let Some(kind) = error {
                        // The value arrived decoded, so the byte beyond the limit is found in the input it was decoded from.
                        let position = start + encoded_length(&input[start..consumed], accepted);
                        return Err(ContentError::new(kind, (offset + position) as u64));
                    }
                }
                FormEvent::PairEnd => {
                    form.end();
                }
                FormEvent::End => return Ok(Stepped::new(consumed, ContentEvent::End)),
                FormEvent::Incomplete => {
                    return Ok(Stepped::new(consumed, ContentEvent::Incomplete));
                }
            }
        }
    }

    /// Check that the content ended, as [`FormParser::parse_end`] reports, once its input has ended. `offset` is where the parser stopped.
    pub(super) fn finish(&self, offset: usize) -> Result<(), ContentError> {
        if self.parser.is_finished() {
            return Ok(());
        }
        Err(ContentError::new(
            ContentErrorKind::UnexpectedEnd,
            offset as u64,
        ))
    }
}

/// Refuse the content with an error `base-web-url` reported at a call whose input, `rest`, began at `start` in the content.
///
/// A value's piece is decoded whole, so a malformed escape or invalid UTF-8 in it can follow a byte beyond the field's limits, which a piece divided before the error reports first. So that the result does not depend on how the content is split, the limit is reported when the bytes before the error exceed it.
fn refuse<H>(form: &Form<H>, rest: &[u8], start: usize, error: FormError) -> ContentError {
    let refused = from_form_error(error);
    if refused.kind() != ContentErrorKind::MalformedUrlEncoded || !form.is_receiving() {
        return refused;
    }
    // The bytes before the error are a value's, with only whole escapes, so their decoded length is known.
    let preceding = &rest[..error.offset() as usize - start];
    let (allowance, limit) = form.allowance();
    if decoded_length(preceding) > allowance {
        let position = start + encoded_length(preceding, allowance);
        return ContentError::new(limit, position as u64);
    }
    refused
}

/// The bytes that percent-encoded `encoded`, which has only whole escapes, decodes to.
fn decoded_length(encoded: &[u8]) -> usize {
    let escapes = encoded.iter().filter(|&&byte| byte == b'%').count();
    encoded.len() - 2 * escapes
}

/// The bytes of percent-encoded `encoded` that decode to its first `decoded` bytes: an escape is three bytes for one, and any other byte, `+` included, one.
fn encoded_length(encoded: &[u8], decoded: usize) -> usize {
    let mut length = 0;
    for _ in 0..decoded {
        length += if encoded[length] == b'%' { 3 } else { 1 };
    }
    length
}
