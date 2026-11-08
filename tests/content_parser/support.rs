// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_web_content::{
    Accepted, ContentError, ContentEvent, ContentHandles, ContentParser, Rejected, Rejection,
};
use base_web_media_type::MediaType;
use std::fmt::Debug;

/// The handle the tests store each upload with: its position among the uploads, counting from 1.
pub type Handle = u32;

pub const BOUNDARY: &str = "XyZ";
pub const MEDIA_TYPE: &str = "multipart/form-data; boundary=XyZ";

/// A part with a `form-data` name and a value.
pub fn field(name: &str, value: &str) -> Vec<u8> {
    part(
        &[(
            "Content-Disposition",
            &format!("form-data; name=\"{name}\""),
        )],
        value.as_bytes(),
    )
}

/// A part with a filename, and a media type when given.
pub fn file(name: &str, filename: &str, media_type: Option<&str>, content: &[u8]) -> Vec<u8> {
    let disposition = format!("form-data; name=\"{name}\"; filename=\"{filename}\"");
    let mut fields = vec![("Content-Disposition", disposition.as_str())];
    fields.extend(media_type.map(|media_type| ("Content-Type", media_type)));
    part(&fields, content)
}

/// A part with these header fields and content, starting with its boundary line.
pub fn part(fields: &[(&str, &str)], content: &[u8]) -> Vec<u8> {
    let mut part = format!("--{BOUNDARY}\r\n").into_bytes();
    for (name, value) in fields {
        part.extend_from_slice(format!("{name}: {value}\r\n").as_bytes());
    }
    part.extend_from_slice(b"\r\n");
    part.extend_from_slice(content);
    part.extend_from_slice(b"\r\n");
    part
}

/// A body of these parts and the closing boundary line.
pub fn body(parts: &[Vec<u8>]) -> Vec<u8> {
    let mut body = parts.concat();
    body.extend_from_slice(format!("--{BOUNDARY}--\r\n").as_bytes());
    body
}

/// What a caller saw: each upload's head, its content reassembled from every piece, its end, and the body's end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Seen {
    Upload {
        path: String,
        filename: String,
        media_type: Option<String>,
    },
    Content(Vec<u8>),
    UploadEnd,
    End,
}

/// Everything a caller learns from parsing a body.
pub struct Run<T> {
    pub seen: Vec<Seen>,
    /// The error `parse` reported, if any.
    pub error: Option<ContentError>,
    /// The bytes fed to the parser when it reported the error, or in all.
    pub fed: usize,
    pub result: Result<Accepted<T, Handle>, Rejected<Handle>>,
    /// The content stored for each handle, from 1.
    pub stored: Vec<Vec<u8>>,
}

impl<T> Run<T> {
    pub fn accepted(self) -> Accepted<T, Handle> {
        match self.result {
            Ok(accepted) => accepted,
            Err(rejected) => panic!("expected the content to be accepted: {}", rejected.reason),
        }
    }

    pub fn rejected(self) -> Rejected<Handle> {
        self.result.expect_err("expected the content to be refused")
    }

    /// The fatal error, which `parse` reported and `finish` returned.
    pub fn error(self) -> ContentError {
        let error = self.error;
        let rejected = self.rejected();
        match rejected.reason {
            Rejection::Error(reported) => {
                assert!(
                    error.is_none_or(|error| error == reported),
                    "finish returns the error parse reported"
                );
                reported
            }
            Rejection::Violations(violations) => panic!("expected an error, got {violations:?}"),
        }
    }

    /// The violations, written as paths and codes.
    pub fn violations(self) -> Vec<String> {
        match self.rejected().reason {
            Rejection::Violations(violations) => {
                violations.iter().map(ToString::to_string).collect()
            }
            Rejection::Error(error) => panic!("expected violations, got {error}"),
        }
    }
}

/// Feed `chunks` through a parser as a caller would: append each to a buffer, parse until more input is needed, store each upload, and drop what was consumed.
pub fn run<T: ContentHandles<Handle>>(media_type: &str, chunks: &[&[u8]]) -> Run<T> {
    let media_type = MediaType::parse(media_type).expect("a valid media type");
    let parser =
        ContentParser::<T, Handle>::new(Some(&media_type)).expect("the media type is accepted");
    run_parser(parser, chunks, b"")
}

/// Give all of `body` to `parse_end` at once, as a caller with the whole body in memory would.
pub fn run_ended<T: ContentHandles<Handle>>(media_type: &str, body: &[u8]) -> Run<T> {
    let media_type = MediaType::parse(media_type).expect("a valid media type");
    let parser =
        ContentParser::<T, Handle>::new(Some(&media_type)).expect("the media type is accepted");
    run_parser(parser, &[], body)
}

/// Feed `chunks` with `parse`, then `rest` with `parse_end`.
pub fn run_parser<T: ContentHandles<Handle>>(
    mut parser: ContentParser<T, Handle>,
    chunks: &[&[u8]],
    rest: &[u8],
) -> Run<T> {
    let (mut buffer, mut seen, mut stored): (Vec<u8>, Vec<Seen>, Vec<Vec<u8>>) = Default::default();
    let (mut fed, mut error, mut open) = (0, None, false);
    // Each chunk as it arrives, then the end of the input.
    let pieces = chunks
        .iter()
        .map(|chunk| (*chunk, false))
        .chain([(rest, true)]);
    'feeding: for (chunk, end) in pieces {
        buffer.extend_from_slice(chunk);
        fed += chunk.len();
        loop {
            let result = if end {
                parser.parse_end(&buffer)
            } else {
                parser.parse(&buffer)
            };
            let parsed = match result {
                Ok(parsed) => parsed,
                Err(reported) => {
                    assert_eq!(
                        parser.parse(&buffer),
                        Err(reported),
                        "the error is reported again"
                    );
                    assert_eq!(parser.parse_end(b"more"), Err(reported), "whatever follows");
                    error = Some(reported);
                    break 'feeding;
                }
            };
            assert!(parsed.consumed <= buffer.len());
            let waiting = match parsed.event {
                ContentEvent::Upload(head) => {
                    assert!(!open);
                    open = true;
                    seen.push(Seen::Upload {
                        path: head.path().to_string(),
                        filename: head.untrusted_filename().to_owned(),
                        media_type: head.untrusted_media_type().map(ToString::to_string),
                    });
                    stored.push(Vec::new());
                    false
                }
                ContentEvent::UploadContent(content) => {
                    assert!(open && !content.is_empty());
                    stored.last_mut().unwrap().extend_from_slice(content);
                    match seen.last_mut() {
                        Some(Seen::Content(seen)) => seen.extend_from_slice(content),
                        _ => seen.push(Seen::Content(content.to_vec())),
                    }
                    false
                }
                ContentEvent::UploadEnd => {
                    assert!(open);
                    open = false;
                    seen.push(Seen::UploadEnd);
                    parser.store(stored.len() as Handle);
                    false
                }
                ContentEvent::End => {
                    if seen.last() != Some(&Seen::End) {
                        seen.push(Seen::End);
                    }
                    // Once the input has ended, the end is final; before, parse on while input remains, so that the epilogue is consumed.
                    end || parsed.consumed == buffer.len()
                }
                ContentEvent::Incomplete => {
                    assert!(!end, "parse_end never reports Incomplete");
                    true
                }
            };
            buffer.drain(..parsed.consumed);
            if waiting {
                break;
            }
        }
    }
    // An error in the epilogue after the end is reported by the next call.
    if error.is_none() {
        error = parser.parse_end(&buffer).err();
    }
    Run {
        seen,
        error,
        fed,
        result: parser.finish(),
        stored,
    }
}

/// Every way of splitting the body in two, and one byte at a time, gives the same events, result, and error as the whole body at once. Returns the run of the whole body.
pub fn split_invariant<T: ContentHandles<Handle> + Debug>(body: &[u8]) -> Run<T> {
    split_invariant_as(MEDIA_TYPE, body)
}

/// As [`split_invariant`], for content of this media type.
pub fn split_invariant_as<T: ContentHandles<Handle> + Debug>(
    media_type: &str,
    body: &[u8],
) -> Run<T> {
    let expected = run::<T>(media_type, &[body]);
    let summary = |run: &Run<T>| {
        let result = match &run.result {
            Ok(accepted) => format!(
                "{:?} {:?} {:?}",
                accepted.content, accepted.discarded, run.stored
            ),
            Err(rejected) => format!("{:?} {:?}", rejected.reason, rejected.handles),
        };
        (run.seen.clone(), run.error, result)
    };
    let expected_summary = summary(&expected);
    for split in 0..=body.len() {
        let (first, second) = body.split_at(split);
        assert_eq!(
            summary(&run(media_type, &[first, second])),
            expected_summary,
            "split at {split}"
        );
        let media = MediaType::parse(media_type).expect("a valid media type");
        let parser =
            ContentParser::<T, Handle>::new(Some(&media)).expect("the media type is accepted");
        assert_eq!(
            summary(&run_parser(parser, &[first], second)),
            expected_summary,
            "split at {split}, with the rest given at the end"
        );
    }
    let bytes: Vec<&[u8]> = body.chunks(1).collect();
    assert_eq!(
        summary(&run(media_type, &bytes)),
        expected_summary,
        "a byte at a time"
    );
    assert_eq!(
        summary(&run_ended(media_type, body)),
        expected_summary,
        "all at the end"
    );
    expected
}

/// The error, and the bytes fed a byte at a time when the parser reported it.
pub fn refused_at<T: ContentHandles<Handle> + Debug>(body: &[u8]) -> (ContentError, usize) {
    refused_at_as::<T>(MEDIA_TYPE, body)
}

/// As [`refused_at`], for content of this media type.
pub fn refused_at_as<T: ContentHandles<Handle> + Debug>(
    media_type: &str,
    body: &[u8],
) -> (ContentError, usize) {
    let bytes: Vec<&[u8]> = body.chunks(1).collect();
    let run = run::<T>(media_type, &bytes);
    let fed = run.fed;
    let error = run.error.expect("parse reports an error");
    (error, fed)
}
