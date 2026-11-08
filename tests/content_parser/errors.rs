// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::Signup;
use crate::support::{
    Handle, MEDIA_TYPE, Seen, body, field, file, part, refused_at, split_invariant,
};
use base_web_content::{Content, ContentError, ContentErrorKind, ContentFormat, ContentParser};
use base_web_media_type::MediaType;

fn new<T: base_web_content::ContentHandles<Handle>>(
    media_type: Option<&str>,
) -> Result<ContentParser<T, Handle>, ContentError> {
    let media_type = media_type.map(|text| MediaType::parse(text).expect("a valid media type"));
    ContentParser::new(media_type.as_ref())
}

#[derive(Content, Debug)]
#[content(formats("application/x-www-form-urlencoded"))]
struct Query {
    _q: String,
}

#[derive(Content, Debug)]
#[content(formats("multipart/form-data"))]
struct Form {
    _q: String,
}

#[test]
fn refuses_media_types_before_reading_content() {
    fn kind<T>(result: Result<ContentParser<T, Handle>, ContentError>) -> (ContentErrorKind, u16) {
        let error = result.map(|_| ()).expect_err("the media type is refused");
        assert_eq!(error.offset(), 0);
        (error.kind(), error.status())
    }
    assert_eq!(
        kind(new::<Signup>(None)),
        (ContentErrorKind::MissingMediaType, 415)
    );
    assert_eq!(
        kind(new::<Signup>(Some("application/json"))),
        (ContentErrorKind::UnsupportedMediaType, 415)
    );
    assert_eq!(
        kind(new::<Signup>(Some("multipart/mixed; boundary=b"))),
        (ContentErrorKind::UnsupportedMediaType, 415)
    );
    assert_eq!(
        kind(new::<Query>(Some(MEDIA_TYPE))),
        (ContentErrorKind::UnacceptedFormat, 415)
    );
    assert_eq!(
        kind(new::<Form>(Some("application/x-www-form-urlencoded"))),
        (ContentErrorKind::UnacceptedFormat, 415)
    );
    for charset in ["iso-8859-1", "utf-16", ""] {
        let media_type = format!("application/x-www-form-urlencoded; charset=\"{charset}\"");
        assert_eq!(
            kind(new::<Signup>(Some(&media_type))),
            (ContentErrorKind::UnsupportedCharset, 415),
            "{charset}"
        );
    }
    for charset in ["UTF-8", "us-ascii"] {
        let media_type = format!("application/x-www-form-urlencoded; charset={charset}");
        let parser = new::<Signup>(Some(&media_type)).expect("the character set is supported");
        assert_eq!(parser.format(), ContentFormat::UrlEncoded);
    }
    assert_eq!(
        kind(new::<Signup>(Some("multipart/form-data"))),
        (ContentErrorKind::InvalidBoundary, 400)
    );
    let long = format!("multipart/form-data; boundary={}", "b".repeat(71));
    assert_eq!(
        kind(new::<Signup>(Some(&long))),
        (ContentErrorKind::InvalidBoundary, 400)
    );
    let parser = new::<Form>(Some("Multipart/Form-Data; Boundary=b"))
        .expect("media types are case-insensitive");
    assert_eq!(parser.format(), ContentFormat::MultipartFormData);
    assert_eq!(
        Some(parser.body_size_limit()),
        Form::body_size_limit(ContentFormat::MultipartFormData)
    );
    assert_eq!(parser.minimum_buffer_size(), 8 * 1024 + 2);
}

/// Check that the body is refused with this kind, at the offset of `at` in it, however it is split.
#[track_caller]
fn refused(body: &[u8], kind: ContentErrorKind, at: &[u8]) {
    let offset = body
        .windows(at.len())
        .position(|window| window == at)
        .expect("the body contains it");
    refused_at_offset(body, kind, offset);
}

#[track_caller]
fn refused_at_offset(body: &[u8], kind: ContentErrorKind, offset: usize) {
    let run = split_invariant::<Signup>(body);
    assert!(
        !run.seen.iter().any(|seen| matches!(seen, Seen::Content(_))),
        "no content was reported"
    );
    let error = run.error();
    assert_eq!(
        (error.kind(), error.offset(), error.status()),
        (kind, offset as u64, 400)
    );
}

#[test]
fn refuses_parts_without_a_form_data_name() {
    for disposition in [
        None,
        Some("attachment; name=\"name\""),
        Some("form-data"),
        Some("form-data; filename=\"a.txt\""),
    ] {
        let fields: Vec<(&str, &str)> = disposition
            .map(|value| ("Content-Disposition", value))
            .into_iter()
            .collect();
        let mut fields = fields;
        fields.push(("Content-Type", "text/plain"));
        let body = body(&[field("name", "Ada"), part(&fields, b"x")]);
        // The second part's header section begins after the first part and its boundary line.
        refused_at_offset(
            &body,
            ContentErrorKind::MissingName,
            field("name", "Ada").len() + 7,
        );
    }
    let body = body(&[
        part(
            &[("Content-Disposition", "form-data; name=\"name\"")],
            b"Ada",
        ),
        part(&[], b""),
    ]);
    let error = split_invariant::<Signup>(&body).error();
    assert_eq!(
        error.kind(),
        ContentErrorKind::MissingName,
        "a part with no header fields at all"
    );
}

#[test]
fn refuses_header_fields_that_cannot_be_read() {
    let disposition = ("Content-Disposition", "form-data; name=\"name\"");
    for fields in [
        vec![disposition, ("Content-Transfer-Encoding", "base64")],
        vec![
            disposition,
            ("Content-Type", "text/plain"),
            ("Content-Type", "text/html"),
        ],
        vec![disposition, ("Content-Type", "not a media type")],
        vec![("Content-Disposition", "form-data; name=name; name=again")],
        vec![("Content-Disposition", "form-data; name*=utf-8''name")],
    ] {
        let body = body(&[part(&fields, b"Ada")]);
        refused(&body, ContentErrorKind::InvalidPartHeader, b"Content-");
    }
}

#[test]
fn refuses_undeclared_names_before_their_content() {
    let body = body(&[
        field("name", "Ada"),
        file("passport", "scan.png", Some("image/png"), b"secret"),
    ]);
    refused(
        &body,
        ContentErrorKind::UnknownName,
        b"Content-Disposition: form-data; name=\"passport\"",
    );
    let section = b"Content-Disposition: form-data; name=\"passport\"; filename=\"scan.png\"\r\nContent-Type: image/png\r\n";
    let start = body
        .windows(section.len())
        .position(|window| window == section)
        .unwrap();
    let (_, fed) = refused_at::<Signup>(&body);
    assert_eq!(
        fed,
        start + section.len() + 2,
        "refused when its header section ends"
    );
}

#[test]
fn reports_malformed_framing_from_base_web_multipart() {
    let body = body(&[part(
        &[
            ("Content-Disposition", "form-data; name=\"name\""),
            ("Bad Name", "x"),
        ],
        b"Ada",
    )]);
    refused(
        &body,
        ContentErrorKind::InvalidPartHeader,
        b"Content-Disposition",
    );
    let mut body = body.clone();
    body.truncate(10);
    let run = split_invariant::<Signup>(&body);
    let error = run.error();
    assert_eq!(
        (error.kind(), error.offset(), error.status()),
        (ContentErrorKind::UnexpectedEnd, 7, 400),
        "an unfinished body is refused when its input ends"
    );
    assert_eq!(
        error.to_string(),
        "content ends before it is complete at byte 7"
    );
    let delimiter = [
        field("name", "Ada"),
        b"\r\n--XyZ".to_vec(),
        b"junk".to_vec(),
    ]
    .concat();
    let error = split_invariant::<Signup>(&delimiter).error();
    assert_eq!(error.kind(), ContentErrorKind::MalformedMultipart);
    assert_eq!(
        error.to_string(),
        format!("multipart body is malformed at byte {}", error.offset())
    );
}

#[test]
fn a_body_may_end_after_its_closing_delimiter() {
    let mut body = body(&[field("name", "Ada")]);
    body.truncate(body.len() - 2);
    assert_eq!(
        split_invariant::<Signup>(&body).accepted().content.name,
        "Ada"
    );
}
