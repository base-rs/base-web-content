// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! `application/x-www-form-urlencoded` content, parsed with `base-web-url`, into the same declarations as multipart bodies.

use crate::support::{Handle, Seen, refused_at_as, run, split_invariant_as};
use base_web_content::{
    Content, ContentErrorKind, ContentEvent, ContentFormat, ContentParser, Rejection, Upload,
};
use base_web_media_type::MediaType;
use base_web_url::FormErrorKind;

const MEDIA_TYPE: &str = "application/x-www-form-urlencoded";

#[derive(Content, Debug, PartialEq)]
struct Range {
    from: Option<String>,
    to: Option<String>,
}

#[derive(Content, Debug, PartialEq)]
struct Sort {
    field: String,
    descending: Option<bool>,
}

#[derive(Content, Debug)]
#[content(pair_count_limit = 6, depth_limit = 3, content_size_limit = 40)]
struct Search {
    #[content(size_limit = 8)]
    q: String,
    page: Option<u32>,
    exact: Option<bool>,
    #[content(count_limit = 3)]
    tags: Option<Vec<String>>,
    range: Option<Range>,
    #[content(count_limit = 2)]
    sorts: Option<Vec<Sort>>,
    // Declared to check that a URL-encoded value is not an upload.
    #[expect(dead_code, reason = "only its violation is checked")]
    file: Option<Upload<Handle>>,
}

fn parse(body: &str) -> crate::support::Run<Search> {
    split_invariant_as::<Search>(MEDIA_TYPE, body.as_bytes())
}

#[test]
fn parses_every_kind_of_field_however_the_content_is_split() {
    let run = parse(
        "&q=rust+go&page=%2B2&tags[]=a&tags%5B%5D=b%26c&range[from]=2020&sorts[][field]=name&&",
    );
    assert_eq!(
        run.seen,
        [Seen::End],
        "URL-encoded content has no uploads to report"
    );
    let search = run.accepted().content;
    assert_eq!(search.q, "rust go");
    assert_eq!((search.page, search.exact), (Some(2), None));
    assert_eq!(
        search.tags,
        Some(vec!["a".to_owned(), "b&c".to_owned()]),
        "names are read after decoding"
    );
    assert_eq!(
        search.range,
        Some(Range {
            from: Some("2020".to_owned()),
            to: None
        })
    );
    assert_eq!(
        search.sorts,
        Some(vec![Sort {
            field: "name".to_owned(),
            descending: None
        }])
    );
}

#[test]
fn groups_array_elements_as_multipart_bodies_do() {
    let search = parse("q=x&sorts[][field]=name&sorts[][descending]=true&sorts[][field]=date")
        .accepted()
        .content;
    assert_eq!(
        search.sorts,
        Some(vec![
            Sort {
                field: "name".to_owned(),
                descending: Some(true)
            },
            Sort {
                field: "date".to_owned(),
                descending: None
            },
        ])
    );
}

#[test]
fn a_pair_without_a_value_is_an_invalid_type() {
    assert_eq!(
        parse("q=").accepted().content.q,
        "",
        "an empty value is a value"
    );
    assert_eq!(
        parse("page&exact=&tags[]&sorts[][field]&file=data&range[to]").violations(),
        [
            "q: required",
            "page: invalid_type",
            "exact: invalid_type",
            "tags[0]: invalid_type",
            "range[to]: invalid_type",
            "sorts[0][field]: invalid_type",
            "file: invalid_type",
        ]
    );
    assert_eq!(
        parse("q").violations(),
        ["q: invalid_type"],
        "a required field without a value is not also missing"
    );
}

#[test]
fn reads_values_in_the_declared_character_set() {
    let ascii = "application/x-www-form-urlencoded; charset=US-ASCII";
    let run = split_invariant_as::<Search>(ascii, b"q=caf%C3%A9");
    assert_eq!(run.violations(), ["q: invalid_type"]);
    let run = split_invariant_as::<Search>(
        "application/x-www-form-urlencoded; charset=utf-8",
        b"q=caf%C3%A9",
    );
    assert_eq!(run.accepted().content.q, "café");
}

/// Check that the content is refused with this kind and status at this offset, however it is split, and, fed a byte at a time, as soon as the byte at `decided` arrives.
#[track_caller]
fn refused(body: &str, kind: ContentErrorKind, status: u16, offset: usize, decided: usize) {
    let error = parse(body).error();
    assert_eq!(
        (error.kind(), error.status(), error.offset()),
        (kind, status, offset as u64)
    );
    let (byte_error, fed) = refused_at_as::<Search>(MEDIA_TYPE, body.as_bytes());
    assert_eq!(byte_error, error);
    assert_eq!(
        fed,
        decided + 1,
        "refused when the byte at {decided} arrives"
    );
}

#[test]
fn refuses_names_the_declaration_does_not_accept_before_their_values() {
    // The name is complete at its `=`.
    refused(
        "q=1&secret=hunter2",
        ContentErrorKind::UnknownName,
        400,
        4,
        10,
    );
    refused("q=1&secret&q=2", ContentErrorKind::UnknownName, 400, 4, 10);
    for name in [
        "tags",
        "tags[0]",
        "range",
        "range[]",
        "sorts[]",
        "q[]",
        "Q",
        "range[from][]",
        "%5Bq%5D",
    ] {
        let error = parse(&format!("{name}=1")).error();
        assert_eq!(
            (error.kind(), error.offset()),
            (ContentErrorKind::UnknownName, 0),
            "{name}"
        );
    }
    assert_eq!(
        parse("%71=decoded").accepted().content.q,
        "decoded",
        "a name is matched once decoded"
    );
}

#[test]
fn refuses_a_name_longer_than_any_the_declaration_accepts() {
    // `sorts[][descending]` is the longest name, of 19 bytes, which percent-encoding could make 57.
    let name = "x".repeat(57);
    refused(
        &format!("{name}=1"),
        ContentErrorKind::UnknownName,
        400,
        0,
        57,
    );
    let long = "x".repeat(58);
    refused(
        &format!("q=1&{long}=1"),
        ContentErrorKind::UnknownName,
        400,
        4 + 57,
        4 + 57,
    );
}

#[test]
fn refuses_malformed_content() {
    let malformed = |_: FormErrorKind| ContentErrorKind::MalformedUrlEncoded;
    refused(
        "q=%zz",
        malformed(FormErrorKind::PercentEncoding),
        400,
        2,
        3,
    );
    refused("q=%ff", malformed(FormErrorKind::Utf8), 400, 2, 4);
    // A name is decoded once it is whole, at its `=`.
    refused(
        "%g1=x",
        malformed(FormErrorKind::PercentEncoding),
        400,
        0,
        3,
    );
    let error = parse("q=1%").error();
    assert_eq!(
        (error.kind(), error.offset()),
        (malformed(FormErrorKind::PercentEncoding), 3),
        "an escape cut short by the end"
    );
}

#[test]
fn limits_the_depth_of_names() {
    assert!(
        parse("q=1&sorts[][field]=x").result.is_ok(),
        "three components"
    );
    refused(
        "q=1&range[from][x][y]=1",
        ContentErrorKind::DepthLimit,
        413,
        4,
        4 + "range[from][x][y]=".len() - 1,
    );
}

#[test]
fn limits_the_size_of_fields() {
    assert_eq!(parse("q=12345678").accepted().content.q, "12345678");
    refused("q=123456789", ContentErrorKind::FieldSizeLimit, 413, 10, 10);
    // The byte beyond the limit is the ninth decoded one, whose escape begins at 26 and is decoded once whole.
    let escaped = "q=%31%32%33%34%35%36%37%38%39";
    refused(escaped, ContentErrorKind::FieldSizeLimit, 413, 26, 28);
}

#[test]
fn limits_the_content_of_every_field() {
    let within = "q=12345678&q=12345678&q=12345678&q=12345678&q=12345678";
    assert_eq!(parse(within).accepted().content.q, "12345678");
    let over = format!("{within}&q=1");
    refused(
        &over,
        ContentErrorKind::ContentSizeLimit,
        413,
        over.len() - 1,
        over.len() - 1,
    );
}

#[test]
fn limits_the_pairs() {
    let within = "q=1&&page=2&q=3&q=4&q=5&q=6&";
    assert_eq!(
        parse(within).accepted().content.q,
        "6",
        "empty pairs are not counted"
    );
    let over = format!("{within}q=7");
    refused(
        &over,
        ContentErrorKind::PairCountLimit,
        413,
        within.len(),
        within.len(),
    );
}

#[test]
fn limits_the_elements_of_arrays() {
    let within = "q=x&tags[]=1&tags[]=2&tags[]=3";
    assert_eq!(
        parse(within).accepted().content.tags.map(|tags| tags.len()),
        Some(3)
    );
    let over = format!("{within}&tags[]=4");
    let start = within.len() + 1;
    refused(
        &over,
        ContentErrorKind::CountLimit,
        413,
        start,
        start + "tags[]=".len() - 1,
    );
}

#[derive(Content, Debug)]
struct Short {
    #[content(size_limit = 4)]
    a: Option<String>,
}

#[test]
fn limits_the_whole_content() {
    // One pair: three times the name and the value, `=`, and `&`.
    let maximum = 3 + 3 * 4 + 2;
    assert_eq!(
        Short::body_size_limit(ContentFormat::UrlEncoded),
        Some(maximum)
    );
    let largest = "&&a=%31%32%33%34&";
    assert_eq!(largest.len(), maximum);
    let run = split_invariant_as::<Short>(MEDIA_TYPE, largest.as_bytes());
    assert_eq!(run.accepted().content.a.as_deref(), Some("1234"));
    // Repeating the field is within its limits, but not within the whole content's.
    let repeated = "a=1&a=2&a=3&a=4&a=5";
    let error = split_invariant_as::<Short>(MEDIA_TYPE, repeated.as_bytes()).error();
    assert_eq!(
        (error.kind(), error.offset(), error.status()),
        (ContentErrorKind::BodySizeLimit, maximum as u64, 413)
    );
    let (_, fed) = refused_at_as::<Short>(MEDIA_TYPE, repeated.as_bytes());
    assert_eq!(fed, maximum + 1);
}

#[test]
fn the_end_is_given_with_parse_end() {
    let media_type = MediaType::parse(MEDIA_TYPE).unwrap();
    let mut parser = ContentParser::<Search, Handle>::new(Some(&media_type))
        .unwrap()
        .with_parameter_rules(base_web_multipart::ParameterRules::Mime);
    assert_eq!(
        parser.format(),
        ContentFormat::UrlEncoded,
        "parameter rules apply only to multipart bodies"
    );
    assert_eq!(
        Some(parser.body_size_limit()),
        Search::body_size_limit(ContentFormat::UrlEncoded)
    );
    assert_eq!(
        parser.minimum_buffer_size(),
        3 * "sorts[][descending]".len() + 1
    );
    let input = b"q=last";
    let parsed = parser.parse(input).unwrap();
    assert_eq!(parsed.event, ContentEvent::Incomplete);
    let Rejection::Error(error) = parser.finish().unwrap_err().reason else {
        panic!("an error")
    };
    assert_eq!(
        (error.kind(), error.offset(), error.status()),
        (ContentErrorKind::UnexpectedEnd, 6, 400),
        "the last pair is only complete when the end is given"
    );
    let run = run::<Search>(MEDIA_TYPE, &[b"q=la", b"st"]);
    assert_eq!(run.accepted().content.q, "last");
}

#[test]
fn never_shows_submitted_content() {
    let media_type = MediaType::parse(MEDIA_TYPE).unwrap();
    for body in [
        "q=hunter2&page=4111",
        "q=hunter2&credit_card=4111",
        "q=%zzhunter2",
        "page=hunter2",
    ] {
        let mut parser = ContentParser::<Search, Handle>::new(Some(&media_type)).unwrap();
        let mut shown = vec![format!("{parser:?}")];
        let mut input = body.as_bytes();
        loop {
            match parser.parse_end(input) {
                Ok(parsed) => {
                    shown.push(format!("{parsed:?}"));
                    input = &input[parsed.consumed..];
                    if parsed.event == ContentEvent::End {
                        break;
                    }
                }
                Err(error) => {
                    shown.push(format!("{error} {error:?}"));
                    break;
                }
            }
        }
        match parser.finish() {
            Ok(accepted) => shown.push(format!("{accepted:?}")),
            Err(rejected) => shown.push(format!("{rejected:?} {}", rejected.reason)),
        }
        for text in &shown {
            for submitted in ["hunter2", "4111", "credit_card"] {
                assert!(!text.contains(submitted), "{submitted:?} in {text}");
            }
        }
    }
}

#[derive(Content, Debug)]
#[content(content_size_limit = 2)]
struct Tiny {
    #[content(size_limit = 1)]
    a: Option<String>,
    b: Option<String>,
}

/// A value is decoded a piece at a time, so a malformed escape or invalid UTF-8 can be in the same piece as a byte beyond a limit; the limit, which comes first, is reported however the content is split.
#[test]
fn reports_a_limit_before_a_later_malformed_byte() {
    let tiny = split_invariant_as::<Tiny>(MEDIA_TYPE, b"a=x&b=y")
        .accepted()
        .content;
    assert_eq!(
        (tiny.a.as_deref(), tiny.b.as_deref()),
        (Some("x"), Some("y"))
    );
    for (body, kind, offset) in [
        (&b"a=aa%ZZ"[..], ContentErrorKind::FieldSizeLimit, 3),
        (b"a=aa\xff", ContentErrorKind::FieldSizeLimit, 3),
        (b"a=%61%61%ZZ", ContentErrorKind::FieldSizeLimit, 5),
        (b"a=x&b=yy%ZZ", ContentErrorKind::ContentSizeLimit, 7),
        (b"a=%ZZaa", ContentErrorKind::MalformedUrlEncoded, 2),
        (b"b=x%ZZ", ContentErrorKind::MalformedUrlEncoded, 3),
    ] {
        let error = split_invariant_as::<Tiny>(MEDIA_TYPE, body).error();
        assert_eq!((error.kind(), error.offset()), (kind, offset), "{body:?}");
    }
}

#[test]
fn a_long_name_is_refused_before_its_depth_is_known() {
    // The name is longer than any the declaration accepts could be, which is decided before it is read whole.
    let name = format!("range{}", "[x]".repeat(20));
    let error = parse(&format!("{name}=1")).error();
    assert_eq!(
        (error.kind(), error.offset()),
        (ContentErrorKind::UnknownName, 57)
    );
}

#[derive(Content, Debug)]
#[content(url_encoded_size_limit = 20)]
struct Bounded {
    a: Option<String>,
    secret: Option<Vec<u8>>,
}

#[test]
fn limits_url_encoded_content() {
    assert_eq!(
        Bounded::body_size_limit(ContentFormat::UrlEncoded),
        Some(20)
    );
    let within = "a=12345678901234567&";
    assert_eq!(
        split_invariant_as::<Bounded>(MEDIA_TYPE, within.as_bytes())
            .accepted()
            .content
            .a
            .as_deref(),
        Some("12345678901234567")
    );
    let error = split_invariant_as::<Bounded>(MEDIA_TYPE, b"a=1234567890123456789").error();
    assert_eq!(
        (error.kind(), error.offset(), error.status()),
        (ContentErrorKind::BodySizeLimit, 20, 413)
    );
}

#[test]
fn url_encoded_bytes_must_be_utf_8() {
    let secret = split_invariant_as::<Bounded>(MEDIA_TYPE, b"secret=%00%7F")
        .accepted()
        .content
        .secret;
    assert_eq!(secret.as_deref(), Some(&b"\0\x7f"[..]));
    let error = split_invariant_as::<Bounded>(MEDIA_TYPE, b"secret=%FF").error();
    assert_eq!(
        (error.kind(), error.offset(), error.status()),
        (ContentErrorKind::MalformedUrlEncoded, 7, 400)
    );
}
