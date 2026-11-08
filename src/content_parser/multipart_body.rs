// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use super::Stepped;
use crate::content_error::from_multipart_error;
use crate::declaration::Limits;
use crate::field_value::Charset;
use crate::form::{FieldSource, Form};
use crate::{ContentError, ContentErrorKind, ContentEvent, UploadHead};
use base_message::FieldSection;
use base_web_media_type::MediaType;
use base_web_multipart::{
    Boundary, ContentType, MultipartError, MultipartEvent, MultipartLimits, MultipartParser,
    ParameterRules, PartHead, PartHeaders,
};

/// A `multipart/form-data` body, framed by `base-web-multipart`, whose parts drive the form.
pub(super) struct MultipartBody {
    parser: MultipartParser,
    rules: ParameterRules,
    epilogue_size_limit: usize,
    /// The bytes after the closing boundary line, once it has been read.
    epilogue: Option<usize>,
    /// Whether the current part is an upload whose content is reported to the caller.
    upload: bool,
}

impl MultipartBody {
    pub(super) fn new(media_type: &MediaType, limits: &Limits) -> Result<Self, ContentErrorKind> {
        let boundary =
            Boundary::from_media_type(media_type).map_err(|_| ContentErrorKind::InvalidBoundary)?;
        let parser = MultipartParser::with_limits(
            &boundary,
            MultipartLimits {
                preamble_size_limit: limits.preamble_size_limit,
                header_size_limit: limits.header_size_limit,
                header_count_limit: limits.header_count_limit,
                part_count_limit: limits.part_count_limit,
            },
        );
        Ok(Self {
            parser,
            rules: ParameterRules::for_media_type(media_type),
            epilogue_size_limit: limits.epilogue_size_limit,
            epilogue: None,
            upload: false,
        })
    }

    pub(super) fn set_parameter_rules(&mut self, rules: ParameterRules) {
        self.rules = rules;
    }

    pub(super) fn minimum_buffer_size(&self) -> usize {
        self.parser.minimum_buffer_size()
    }

    /// Parse until there is an event for the caller. `offset` is the offset of `input` in the body, `end` whether the body ends with it, and `ended` whether the input has ended, perhaps beyond the bytes `input` allows.
    pub(super) fn parse<'a, H>(
        &mut self,
        form: &mut Form<H>,
        input: &'a [u8],
        offset: usize,
        end: bool,
        ended: bool,
    ) -> Result<Stepped<'a>, ContentError> {
        let at = |kind, position: usize| ContentError::new(kind, (offset + position) as u64);
        let mut consumed = 0;
        loop {
            let rest = &input[consumed..];
            if let Some(epilogue) = &mut self.epilogue {
                // SECURITY: `base-web-multipart` leaves the epilogue unlimited, since it keeps none of it; design/limits.md limits it here, so that a body cannot continue without bound after it ends.
                let allowed = self.epilogue_size_limit - *epilogue;
                if rest.len() > allowed {
                    return Err(at(ContentErrorKind::EpilogueSizeLimit, consumed + allowed));
                }
                *epilogue += rest.len();
                return Ok(Stepped::new(input.len(), ContentEvent::End));
            }
            let parsed = self.parser.parse(rest).map_err(|error| self.error(error))?;
            consumed += parsed.consumed;
            match parsed.event {
                MultipartEvent::Part(head) => {
                    let start = consumed - head.as_bytes().len();
                    let upload = self.begin(form, head).map_err(|kind| at(kind, start))?;
                    self.upload = upload.is_some();
                    if let Some(head) = upload {
                        return Ok(Stepped::new(consumed, ContentEvent::Upload(head)));
                    }
                }
                MultipartEvent::Content(content) => {
                    let start = consumed - content.len();
                    let (accepted, error) = form.content(content);
                    let error = error.map(|kind| at(kind, start + accepted));
                    if self.upload && accepted > 0 {
                        // Report the content within the limits, and the error on the next call, so the caller sees the same content however the body is split.
                        return Ok(Stepped {
                            consumed: start + accepted,
                            event: ContentEvent::UploadContent(&content[..accepted]),
                            deferred: error,
                        });
                    }
                    if let Some(error) = error {
                        return Err(error);
                    }
                }
                MultipartEvent::PartEnd => {
                    if form.end() {
                        return Ok(Stepped::new(consumed, ContentEvent::UploadEnd));
                    }
                }
                // The epilogue is read on later calls, so that `End` is reported at the closing boundary line however the body is split.
                MultipartEvent::End => {
                    self.epilogue = Some(0);
                    let rest = input.len() - consumed;
                    if !ended || rest == 0 {
                        return Ok(Stepped::new(consumed, ContentEvent::End));
                    }
                    // The rest of the body is the epilogue: consume it now, or report the end and then the epilogue's error, as the next call would if the body were split here.
                    let deferred = (rest > self.epilogue_size_limit).then(|| {
                        at(
                            ContentErrorKind::EpilogueSizeLimit,
                            consumed + self.epilogue_size_limit,
                        )
                    });
                    if deferred.is_none() {
                        self.epilogue = Some(rest);
                    }
                    let consumed = if deferred.is_some() {
                        consumed
                    } else {
                        input.len()
                    };
                    return Ok(Stepped {
                        consumed,
                        event: ContentEvent::End,
                        deferred,
                    });
                }
                MultipartEvent::Incomplete if end => {
                    // The body may end just after its closing `--`; anything else is unfinished.
                    self.finish()?;
                    return Ok(Stepped::new(consumed, ContentEvent::End));
                }
                MultipartEvent::Incomplete => {
                    return Ok(Stepped::new(consumed, ContentEvent::Incomplete));
                }
            }
        }
    }

    /// A part begins: its name, and what its header section says about it, given to the form. Returns the upload's head when it is an upload the caller must store.
    fn begin<H>(
        &self,
        form: &mut Form<H>,
        head: PartHead<'_>,
    ) -> Result<Option<UploadHead>, ContentErrorKind> {
        let headers = PartHeaders::interpret(head).map_err(invalid)?;
        let disposition = headers
            .content_disposition
            .filter(|disposition| disposition.disposition_type() == "form-data")
            .ok_or(ContentErrorKind::MissingName)?;
        let name = disposition
            .name(self.rules)
            .map_err(invalid)?
            .ok_or(ContentErrorKind::MissingName)?;
        let filename = disposition.filename(self.rules).map_err(invalid)?;
        let media_type = headers.content_type.map(ContentType::into_media_type);
        // A part is a file if and only if it has a `filename`. For a declared upload, the form takes an empty one, which browsers send for a file input with no file chosen, to mean no file.
        let source = match &filename {
            Some(filename) => FieldSource::File {
                filename,
                media_type: media_type.as_ref(),
            },
            None => {
                let charset = media_type
                    .as_ref()
                    .and_then(|media_type| media_type.parameter("charset"));
                FieldSource::Value(Charset::from_parameter(charset))
            }
        };
        form.begin(&name, source)
    }

    /// Check that the body was complete, once its input has ended.
    pub(super) fn finish(&self) -> Result<(), ContentError> {
        self.parser.finish().map_err(|error| self.error(error))
    }

    /// A `base-web-multipart` error as this crate's.
    fn error(&self, error: MultipartError) -> ContentError {
        let error = from_multipart_error(error);
        match error.kind() {
            // `base-web-multipart` reports the preamble limit at the start of the preamble, which begins the body. This crate reports it at the first byte beyond the limit, as it reports the size limits it applies itself, which for the preamble is at the limit.
            ContentErrorKind::PreambleSizeLimit => ContentError::new(
                error.kind(),
                self.parser.limits().preamble_size_limit as u64,
            ),
            _ => error,
        }
    }
}

/// A part's header fields cannot be read, whatever the reason, which `base-message` and `base-web-multipart` keep in their errors.
fn invalid<E>(_error: E) -> ContentErrorKind {
    ContentErrorKind::InvalidPartHeader
}
