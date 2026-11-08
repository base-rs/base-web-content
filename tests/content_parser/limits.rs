// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Each limit, at its value and one byte or item over, fed whole, split everywhere, and a byte at a time.

use crate::support::{
    BOUNDARY, Handle, Seen, body, field, file, part, refused_at, split_invariant,
};
use base_web_content::{Content, ContentError, ContentErrorKind, ContentFormat, Upload};

#[derive(Content, Debug)]
#[content(
    preamble_size_limit = 8,
    epilogue_size_limit = 4,
    header_size_limit = 64,
    header_count_limit = 2,
    part_count_limit = 3,
    content_size_limit = 10,
    depth_limit = 2
)]
struct Small {
    #[content(size_limit = 6)]
    a: Option<String>,
    b: Option<String>,
    #[content(size_limit = 6)]
    file: Option<Upload<Handle>>,
    #[content(count_limit = 2)]
    list: Option<Vec<String>>,
}

/// Check that the body is refused with this kind at this offset, however it is split, and, fed a byte at a time, as soon as the byte at `decided` arrives.
#[track_caller]
fn refused(body: &[u8], kind: ContentErrorKind, offset: usize, decided: usize) -> ContentError {
    let error = split_invariant::<Small>(body).error();
    assert_eq!((error.kind(), error.offset()), (kind, offset as u64));
    assert_eq!(error.status(), 413);
    let (byte_error, fed) = refused_at::<Small>(body);
    assert_eq!(byte_error, error);
    assert_eq!(
        fed,
        decided + 1,
        "refused when the byte at {decided} arrives"
    );
    error
}

#[track_caller]
fn accepted(body: &[u8]) -> Small {
    split_invariant::<Small>(body).accepted().content
}

/// The offset of the first occurrence of `needle` in `body`.
fn find(body: &[u8], needle: &[u8]) -> usize {
    body.windows(needle.len())
        .position(|window| window == needle)
        .expect("the body contains it")
}

#[test]
fn field_size_limit() {
    assert_eq!(
        accepted(&body(&[field("a", "123456")])).a.as_deref(),
        Some("123456")
    );
    let body = body(&[field("a", "1234567")]);
    let at = find(&body, b"7");
    refused(&body, ContentErrorKind::FieldSizeLimit, at, at);
}

#[test]
fn upload_size_limit() {
    let within = body(&[file("file", "f", None, b"123456")]);
    assert_eq!(accepted(&within).file.map(|file| file.size()), Some(6));
    let body = body(&[file("file", "f", None, b"1234567")]);
    let at = find(&body, b"7");
    refused(&body, ContentErrorKind::FieldSizeLimit, at, at);
    let run = split_invariant::<Small>(&body);
    assert_eq!(
        run.seen[1..],
        [Seen::Content(b"123456".to_vec())],
        "the content within the limit is reported, however the body is split"
    );
}

#[test]
fn a_refused_field_is_limited_too() {
    let body = body(&[file("a", "f", None, b"1234567")]);
    let at = find(&body, b"7");
    refused(&body, ContentErrorKind::FieldSizeLimit, at, at);
}

#[test]
fn content_size_limit() {
    let within = body(&[field("a", "123456"), field("b", "7890")]);
    assert_eq!(accepted(&within).b.as_deref(), Some("7890"));
    let body = body(&[field("a", "123456"), field("b", "78901")]);
    let at = find(&body, b"1\r\n--XyZ--");
    refused(&body, ContentErrorKind::ContentSizeLimit, at, at);
}

#[test]
fn content_size_limit_counts_repeated_fields() {
    let body = body(&[field("a", "1234"), field("a", "5678"), field("a", "90x")]);
    let at = find(&body, b"x");
    refused(&body, ContentErrorKind::ContentSizeLimit, at, at);
}

#[test]
fn count_limit() {
    let within = body(&[field("list[]", "1"), field("list[]", "2")]);
    assert_eq!(accepted(&within).list.map(|list| list.len()), Some(2));
    let body = body(&[
        field("list[]", "1"),
        field("list[]", "2"),
        field("list[]", "3"),
    ]);
    let header = find(
        &body,
        b"Content-Disposition: form-data; name=\"list[]\"\r\n\r\n3",
    );
    // The name is refused when its header section, with the empty line that ends it, has arrived.
    refused(
        &body,
        ContentErrorKind::CountLimit,
        header,
        header + 47 + 2 - 1,
    );
}

#[test]
fn depth_limit() {
    let body = body(&[field("a[b][c]", "1")]);
    let header = find(&body, b"Content-Disposition");
    let section = "Content-Disposition: form-data; name=\"a[b][c]\"\r\n".len();
    refused(
        &body,
        ContentErrorKind::DepthLimit,
        header,
        header + section + 1,
    );
}

#[test]
fn part_count_limit() {
    let within = body(&[field("a", "1"), field("b", "2"), field("a", "3")]);
    assert_eq!(accepted(&within).a.as_deref(), Some("3"));
    let body = body(&[
        field("a", "1"),
        field("b", "2"),
        field("a", "3"),
        field("b", "4"),
    ]);
    // The part is counted at the line break after its boundary, when it arrives.
    let at = find(
        &body,
        b"--XyZ\r\nContent-Disposition: form-data; name=\"b\"\r\n\r\n4",
    ) + 5;
    refused(&body, ContentErrorKind::PartCountLimit, at, at + 1);
}

#[test]
fn header_size_limit() {
    let disposition = "form-data; name=\"a\"";
    // `Content-Disposition: ` and the line break are 23 bytes, and `X: ` and its line break 5.
    let padding = |length| "p".repeat(64 - 23 - disposition.len() - 5 + length);
    let fields = |length| {
        part(
            &[
                ("Content-Disposition", disposition),
                ("X", &padding(length)),
            ],
            b"1",
        )
    };
    assert_eq!(
        accepted(&body(&[fields(0)])).a.as_deref(),
        Some("1"),
        "a header section of 64 bytes"
    );
    let body = body(&[fields(1)]);
    let start = find(&body, b"Content-Disposition");
    // The section might still end within its limit until its 65th byte and the line break that could follow have arrived.
    refused(
        &body,
        ContentErrorKind::HeaderSizeLimit,
        start,
        start + 64 + 1,
    );
}

#[test]
fn header_count_limit() {
    let fields = |count: usize| {
        let mut fields = vec![("Content-Disposition", "form-data; name=\"a\"")];
        fields.extend(std::iter::repeat_n(("X", "1"), count - 1));
        part(&fields, b"1")
    };
    assert_eq!(accepted(&body(&[fields(2)])).a.as_deref(), Some("1"));
    let body = body(&[fields(3)]);
    let start = find(&body, b"Content-Disposition");
    // The fields are counted when the section is complete, at the last byte of the empty line that ends it.
    let end = find(&body, b"\r\n\r\n1") + 3;
    refused(&body, ContentErrorKind::HeaderCountLimit, start, end);
}

#[test]
fn preamble_size_limit() {
    let preamble = |length: usize| {
        [
            b"p".repeat(length),
            b"\r\n".to_vec(),
            body(&[field("a", "1")]),
        ]
        .concat()
    };
    assert_eq!(
        accepted(&preamble(8)).a.as_deref(),
        Some("1"),
        "the line break before the boundary is the delimiter's"
    );
    refused(&preamble(9), ContentErrorKind::PreambleSizeLimit, 8, 8);
}

#[test]
fn epilogue_size_limit() {
    let epilogue = |text: &str| [body(&[field("a", "1")]), text.as_bytes().to_vec()].concat();
    assert_eq!(accepted(&epilogue("\r\nep")).a.as_deref(), Some("1"));
    let body = epilogue("\r\nepi");
    let at = body.len() - 1;
    refused(&body, ContentErrorKind::EpilogueSizeLimit, at, at);
}

/// One field, with limits that make the largest body the declaration could need easy to reach.
#[derive(Content, Debug)]
#[content(
    preamble_size_limit = 0,
    epilogue_size_limit = 0,
    header_size_limit = 100
)]
struct Repeated {
    #[content(size_limit = 10)]
    a: Option<String>,
}

#[test]
fn body_size_limit() {
    // One part: a boundary line of 76 bytes, a header section of 100, its empty line, 10 bytes of content, and a closing line of 78.
    let maximum = 76 + 100 + 2 + 10 + 78;
    assert_eq!(
        Repeated::body_size_limit(ContentFormat::MultipartFormData),
        Some(maximum)
    );
    let boundary = "b".repeat(70);
    let media_type = format!("multipart/form-data; boundary={boundary}");
    let disposition = "Content-Disposition: form-data; name=\"a\"\r\n";
    let padding = format!("X: {}\r\n", "p".repeat(100 - disposition.len() - 5));
    let largest =
        format!("\r\n--{boundary}\r\n{disposition}{padding}\r\n0123456789\r\n--{boundary}--\r\n");
    assert_eq!(largest.len(), maximum);
    let run = crate::support::run::<Repeated>(&media_type, &[largest.as_bytes()]);
    assert_eq!(run.accepted().content.a.as_deref(), Some("0123456789"));
    // Transport padding after the closing boundary is not counted in the maximum.
    let padded = largest.replace("--\r\n", "-- \r\n");
    let bytes: Vec<&[u8]> = padded.as_bytes().chunks(1).collect();
    for chunks in [vec![padded.as_bytes()], bytes] {
        let run = crate::support::run::<Repeated>(&media_type, &chunks);
        let error = run.error();
        assert_eq!(
            (error.kind(), error.offset()),
            (ContentErrorKind::BodySizeLimit, maximum as u64)
        );
    }
}

#[test]
fn body_size_limit_counts_repeated_fields() {
    let parts: Vec<Vec<u8>> = (0..10).map(|_| field("a", "1")).collect();
    let body = body(&parts);
    let maximum = Repeated::body_size_limit(ContentFormat::MultipartFormData).unwrap();
    assert!(body.len() > maximum);
    let run = split_invariant::<Repeated>(&body);
    let error = run.error();
    assert_eq!(
        (error.kind(), error.offset(), error.status()),
        (ContentErrorKind::BodySizeLimit, maximum as u64, 413)
    );
    let (_, fed) = refused_at::<Repeated>(&body);
    assert_eq!(
        fed,
        maximum + 1,
        "refused when the byte past the maximum arrives"
    );
    let _ = BOUNDARY;
}

/// A boundary after a bare CR or LF is malformed, but the content before the break is read first, as it is when the body is split before the break, so its limits apply however the body is split.
#[test]
fn reads_the_content_before_a_bare_line_break() {
    let field_part =
        |content: &[u8]| part(&[("Content-Disposition", "form-data; name=\"a\"")], content);
    let refused_with = |body: &[u8], kind: ContentErrorKind, at: usize| {
        let run = split_invariant::<Small>(body);
        let seen = run.seen.clone();
        let error = run.error();
        assert_eq!((error.kind(), error.offset()), (kind, at as u64));
        seen
    };
    let within = body(&[field_part(b"12\n--XyZ")]);
    refused_with(
        &within,
        ContentErrorKind::MalformedMultipart,
        find(&within, b"\n--XyZ"),
    );
    let over = body(&[field_part(b"1234567\n--XyZ")]);
    refused_with(&over, ContentErrorKind::FieldSizeLimit, find(&over, b"7"));
    let upload = body(&[file("file", "f", None, b"abc\n--XyZ")]);
    let seen = refused_with(
        &upload,
        ContentErrorKind::MalformedMultipart,
        find(&upload, b"\n--XyZ"),
    );
    assert_eq!(
        seen[1..],
        [Seen::Content(b"abc".to_vec())],
        "the upload's content before the break is reported"
    );
    let upload_over = body(&[file("file", "f", None, b"1234567\r--XyZ")]);
    let seen = refused_with(
        &upload_over,
        ContentErrorKind::FieldSizeLimit,
        find(&upload_over, b"7"),
    );
    assert_eq!(seen[1..], [Seen::Content(b"123456".to_vec())]);
    let preamble = [b"pp\n".to_vec(), body(&[field("a", "1")])].concat();
    refused_with(&preamble, ContentErrorKind::MalformedMultipart, 2);
    let long_preamble = [b"pppppppppppp\n".to_vec(), body(&[field("a", "1")])].concat();
    refused_with(&long_preamble, ContentErrorKind::PreambleSizeLimit, 8);
    let after_boundary = b"--XyZ\n--XyZ--\r\n";
    refused_with(after_boundary, ContentErrorKind::MalformedMultipart, 5);
}

#[test]
fn reports_the_end_before_refusing_the_bytes_beyond_the_maximum() {
    let maximum = Repeated::body_size_limit(ContentFormat::MultipartFormData).unwrap();
    let boundary = "b".repeat(70);
    let media_type = format!("multipart/form-data; boundary={boundary}");
    let disposition = "Content-Disposition: form-data; name=\"a\"\r\n";
    let padding = format!("X: {}\r\n", "p".repeat(100 - disposition.len() - 5));
    // One byte less content, and a byte of padding after the closing boundary, end the body at the maximum; one more byte follows.
    let body =
        format!("\r\n--{boundary}\r\n{disposition}{padding}\r\n012345678\r\n--{boundary}-- \r\nx");
    assert_eq!(body.len(), maximum + 1);
    let run = crate::support::split_invariant_as::<Repeated>(&media_type, body.as_bytes());
    assert_eq!(run.seen, [Seen::End]);
    let error = run.error();
    assert_eq!(
        (error.kind(), error.offset()),
        (ContentErrorKind::BodySizeLimit, maximum as u64)
    );
}

#[test]
fn an_epilogue_beyond_the_largest_body_is_refused_for_its_own_limit() {
    let maximum = Small::body_size_limit(ContentFormat::MultipartFormData).unwrap();
    let mut body = body(&[field("a", "1")]);
    let closing = body.len();
    body.resize(maximum + 1, b'e');
    let error = split_invariant::<Small>(&body).error();
    assert_eq!(
        (error.kind(), error.offset()),
        (ContentErrorKind::EpilogueSizeLimit, (closing + 4) as u64)
    );
}

#[test]
fn a_line_break_before_a_bare_break_in_the_preamble_counts() {
    for (preamble, kind, offset) in [
        (
            &b"ppppppp\r\n\n"[..],
            ContentErrorKind::PreambleSizeLimit,
            8,
        ),
        (b"pppppp\r\n\n", ContentErrorKind::MalformedMultipart, 8),
        (b"ppppppp\r", ContentErrorKind::MalformedMultipart, 7),
    ] {
        let body = [preamble.to_vec(), body(&[field("a", "1")])].concat();
        let error = split_invariant::<Small>(&body).error();
        assert_eq!(
            (error.kind(), error.offset()),
            (kind, offset),
            "{preamble:?}"
        );
    }
}
