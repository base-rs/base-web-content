// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::ParameterRules;
use crate::form::Form;
use crate::{
    Accepted, ContentError, ContentErrorKind, ContentEvent, ContentFormat, ContentHandles,
    ContentParsed, Rejected,
};
use base_web_media_type::MediaType;
use std::fmt;
use std::marker::PhantomData;

mod multipart_body;
mod url_encoded_body;

use multipart_body::MultipartBody;
use url_encoded_body::UrlEncodedBody;

/// A sans-I/O parser for a request's content, into the struct `T` declares, fed bytes as they arrive.
///
/// The format is chosen by the request's media type. Every field and upload is matched against the declaration as it arrives: a name the declaration does not accept, and any limit exceeded, stop parsing with a fatal [`ContentError`] before any more content is read. Fields that arrived but cannot be accepted, and required fields that did not, are collected as [`Violation`](crate::Violation)s and returned together when parsing finishes.
///
/// The caller owns the buffer, as with `base-web-multipart`'s `MultipartParser`. Each call to [`Self::parse`] reports how many bytes it consumed and what they held: the caller drops the consumed bytes, appends whatever arrives next, and calls again with what remains. When the input ends, the caller gives what remains to [`Self::parse_end`] until it reports [`ContentEvent::End`]; then [`Self::finish`] returns the struct, or why it was refused. The result, events, violations, and errors are the same however the body is split, except that an upload's content may arrive in different pieces.
///
/// Fields other than uploads are buffered within their limits. An accepted upload's content is reported to the caller, borrowed from the input, to store wherever it chooses; when the upload ends, the caller gives the parser a handle of type `H` for it, which the result holds in its [`Upload`](crate::Upload). Uploads are stored before the whole body has been validated, so when parsing fails, [`Rejected`] returns every handle the caller gave, for the caller to remove what it stored. An upload whose content was still arriving when parsing failed has no handle, and the caller must remove what it stored of it.
///
/// ```
/// use base_web_content::{Content, ContentEvent, ContentParser, Upload};
/// use base_web_media_type::MediaType;
///
/// #[derive(Content)]
/// struct Comment {
///     body: String,
///     #[content(accept("text/*"))]
///     attachment: Option<Upload<usize>>,
/// }
///
/// let media_type = MediaType::parse("multipart/form-data; boundary=b")?;
/// let body = b"--b\r\nContent-Disposition: form-data; name=\"body\"\r\n\r\nHello!\r\n--b\r\nContent-Disposition: form-data; name=\"attachment\"; filename=\"notes.txt\"\r\nContent-Type: text/plain\r\n\r\nSome notes.\r\n--b--\r\n";
///
/// let mut parser = ContentParser::<Comment, usize>::new(Some(&media_type))?;
/// let mut stored: Vec<Vec<u8>> = Vec::new();
/// // The whole body is in memory, so its end is known.
/// let mut input = &body[..];
/// loop {
///     let parsed = parser.parse_end(input)?;
///     input = &input[parsed.consumed..];
///     match parsed.event {
///         // Begin storing an upload, such as in a temporary file.
///         ContentEvent::Upload(_) => stored.push(Vec::new()),
///         ContentEvent::UploadContent(content) => stored.last_mut().unwrap().extend_from_slice(content),
///         // Give the parser a handle for what was stored.
///         ContentEvent::UploadEnd => parser.store(stored.len() - 1),
///         ContentEvent::End => break,
///         ContentEvent::Incomplete => unreachable!("parse_end is given the rest of the input"),
///     }
/// }
///
/// let comment = parser.finish().expect("the comment is accepted").content;
/// assert_eq!(comment.body, "Hello!");
/// let attachment = comment.attachment.expect("an attachment");
/// assert_eq!(stored[*attachment.handle()], b"Some notes.");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct ContentParser<T, H = ()> {
    form: Form<H>,
    format: Format,
    body_size_limit: usize,
    /// The bytes consumed before the current call, which never exceed the body size limit.
    offset: usize,
    /// The error the parser reported, or will report on the next call, which it reports for every later call.
    failed: Option<ContentError>,
    /// Whether the parser has reported the end of the body.
    ended: bool,
    declaration: PhantomData<fn() -> T>,
}

/// The format being parsed, which drives the form with the fields it finds.
enum Format {
    MultipartFormData(Box<MultipartBody>),
    UrlEncoded(UrlEncodedBody),
}

/// What one step of a format found: the event, the bytes it consumed, and an error to report on the next call, after content within a limit that the bytes after it exceed.
struct Stepped<'a> {
    consumed: usize,
    event: ContentEvent<'a>,
    deferred: Option<ContentError>,
}

impl<'a> Stepped<'a> {
    fn new(consumed: usize, event: ContentEvent<'a>) -> Self {
        Self {
            consumed,
            event,
            deferred: None,
        }
    }
}

impl<T: ContentHandles<H>, H> ContentParser<T, H> {
    /// A parser for content with this media type, the request's `Content-Type`, if it has one.
    ///
    /// `multipart/form-data` is parsed with `base-web-multipart`, reading part names and filenames under the [`ParameterRules`] its media type gives, which [`Self::with_parameter_rules`] can override. `application/x-www-form-urlencoded` is parsed with `base-web-url`, in UTF-8 or US-ASCII as its `charset` parameter says.
    ///
    /// # Errors
    ///
    /// A [`ContentError`] at offset 0 when the media type is missing, is not a format this crate parses, is a format `T` does not accept, is `multipart/form-data` without a valid boundary, or is `application/x-www-form-urlencoded` with a `charset` other than `UTF-8` or `US-ASCII`.
    pub fn new(media_type: Option<&MediaType>) -> Result<Self, ContentError> {
        let error = |kind| ContentError::new(kind, 0);
        let media_type = media_type.ok_or(error(ContentErrorKind::MissingMediaType))?;
        let format = ContentFormat::from_media_type(media_type)
            .ok_or(error(ContentErrorKind::UnsupportedMediaType))?;
        let declaration = &T::DECLARATION;
        let body_size_limit = declaration
            .body_size_limit(format)
            .ok_or(error(ContentErrorKind::UnacceptedFormat))?;
        let format = match format {
            ContentFormat::MultipartFormData => Format::MultipartFormData(Box::new(
                MultipartBody::new(media_type, &declaration.limits).map_err(error)?,
            )),
            ContentFormat::UrlEncoded => {
                Format::UrlEncoded(UrlEncodedBody::new(media_type, declaration).map_err(error)?)
            }
        };
        Ok(Self {
            form: Form::new(declaration),
            format,
            body_size_limit,
            offset: 0,
            failed: None,
            ended: false,
            declaration: PhantomData,
        })
    }
}

impl<T, H> ContentParser<T, H> {
    /// The parser, reading part names and filenames under these rules rather than those of the media type. URL-encoded content has no parameters, so its parser is unchanged.
    pub fn with_parameter_rules(mut self, rules: ParameterRules) -> Self {
        match &mut self.format {
            Format::MultipartFormData(body) => body.set_parameter_rules(rules),
            Format::UrlEncoded(_) => {}
        }
        self
    }

    /// The format being parsed.
    pub fn format(&self) -> ContentFormat {
        match self.format {
            Format::MultipartFormData(_) => ContentFormat::MultipartFormData,
            Format::UrlEncoded(_) => ContentFormat::UrlEncoded,
        }
    }

    /// The largest body the declaration could need in this format. Parsing refuses a longer body with [`ContentErrorKind::BodySizeLimit`].
    pub fn body_size_limit(&self) -> usize {
        self.body_size_limit
    }

    /// The input a caller must be able to buffer: given at least this many bytes, [`Self::parse`] always consumes some or reports an event other than [`ContentEvent::Incomplete`], since a multipart part's header section, and a URL-encoded name, are read whole.
    pub fn minimum_buffer_size(&self) -> usize {
        match &self.format {
            Format::MultipartFormData(body) => body.minimum_buffer_size(),
            Format::UrlEncoded(body) => body.minimum_buffer_size(),
        }
    }

    /// Parse the next event from `input`, which must begin with the bytes the previous call did not consume, while more input may follow. Once the input has ended, call [`Self::parse_end`] instead.
    ///
    /// # Errors
    ///
    /// A fatal [`ContentError`] when the body is malformed, has a name the declaration does not accept, or exceeds a limit. Once the parser reports an error, it reports the same error for every later call.
    ///
    /// # Panics
    ///
    /// When an upload ended and its handle has not been given with [`Self::store`].
    pub fn parse<'a>(&mut self, input: &'a [u8]) -> Result<ContentParsed<'a>, ContentError> {
        self.run(input, false)
    }

    /// Parse the next event from `input`, which must be all of the input that remains after the bytes already consumed, once the request's body has ended. Call it until it reports [`ContentEvent::End`], handling uploads as [`Self::parse`] reports them; it never reports [`ContentEvent::Incomplete`]. URL-encoded content ends only where its input does, so its last pair is only complete here.
    ///
    /// # Errors
    ///
    /// As for [`Self::parse`], and [`ContentErrorKind::UnexpectedEnd`] when the content is not complete, such as a multipart body without its closing boundary. An error in what follows a multipart body's closing boundary, the epilogue, is reported by the call after the one that reports [`ContentEvent::End`], and by [`Self::finish`].
    ///
    /// # Panics
    ///
    /// When an upload ended and its handle has not been given with [`Self::store`].
    pub fn parse_end<'a>(&mut self, input: &'a [u8]) -> Result<ContentParsed<'a>, ContentError> {
        self.run(input, true)
    }

    fn run<'a>(&mut self, input: &'a [u8], end: bool) -> Result<ContentParsed<'a>, ContentError> {
        if let Some(error) = self.failed {
            return Err(error);
        }
        assert!(
            !self.form.is_unstored(),
            "an upload ended, and its handle must be given to `store` before parsing on"
        );
        // Bytes past the largest body the declaration could need are never parsed, so a longer body is refused at that byte, however it is split.
        let allowed = input.len().min(self.body_size_limit - self.offset);
        // A body cut short at the maximum has not ended there, so it is parsed as if more may follow, and refused below.
        let ended_input = end;
        let end = end && allowed == input.len();
        let (form, input_allowed, offset) = (&mut self.form, &input[..allowed], self.offset);
        let stepped = match &mut self.format {
            Format::MultipartFormData(body) => {
                body.parse(form, input_allowed, offset, end, ended_input)
            }
            Format::UrlEncoded(body) => body.parse(form, input_allowed, offset, end),
        };
        let beyond =
            ContentError::new(ContentErrorKind::BodySizeLimit, self.body_size_limit as u64);
        let ended = self.ended;
        let stepped = stepped.and_then(|mut stepped| {
            if allowed == input.len() {
                return Ok(stepped);
            }
            match stepped.event {
                // The parser needs more than the bytes allowed, or has consumed every byte after the end it reported, and more follow.
                ContentEvent::Incomplete => Err(beyond),
                ContentEvent::End if ended && stepped.consumed == allowed => Err(beyond),
                // The end is reported where the body reaches it, and the bytes beyond the maximum refused after it, as they would be if the body were split there; once the input has ended, the next call would be the last, so the error is reported then.
                ContentEvent::End if ended_input => {
                    stepped.deferred = stepped.deferred.or(Some(beyond));
                    Ok(stepped)
                }
                _ => Ok(stepped),
            }
        });
        match stepped {
            Ok(stepped) => {
                self.offset += stepped.consumed;
                self.failed = stepped.deferred;
                self.ended |= stepped.event == ContentEvent::End;
                Ok(ContentParsed {
                    consumed: stepped.consumed,
                    event: stepped.event,
                })
            }
            Err(error) => {
                self.failed = Some(error);
                Err(error)
            }
        }
    }

    /// Give the handle for what the caller stored of the upload that just ended.
    ///
    /// # Panics
    ///
    /// When no upload has ended since the last handle was given.
    pub fn store(&mut self, handle: H) {
        self.form.store(handle);
    }

    /// Finish once the input has ended and [`Self::parse_end`] has reported [`ContentEvent::End`]: the declared struct, or why it was refused, with every handle the caller gave.
    ///
    /// # Errors
    ///
    /// [`Rejected`], with a fatal [`ContentError`] when parsing failed or the body ended early, or every [`Violation`](crate::Violation) when it parsed but some fields cannot be accepted.
    pub fn finish(self) -> Result<Accepted<T, H>, Rejected<H>>
    where
        T: ContentHandles<H>,
    {
        let error = self.failed.or_else(|| match &self.format {
            Format::MultipartFormData(body) => body.finish().err(),
            Format::UrlEncoded(body) => body.finish(self.offset).err(),
        });
        self.form.finish(error)
    }
}

/// Shows the format, the offset, and any error, but no content.
impl<T, H> fmt::Debug for ContentParser<T, H> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ContentParser")
            .field("format", &self.format())
            .field("offset", &self.offset)
            .field("body_size_limit", &self.body_size_limit)
            .field("failed", &self.failed)
            .finish_non_exhaustive()
    }
}
