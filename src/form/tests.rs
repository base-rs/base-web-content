// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use super::{FieldSource, Form};
use crate::__private::{Declaration, Item};
use crate::field_value::Charset;
use crate::{
    Accepted, Content, ContentErrorKind, FieldPathSegment, FieldValue, Rejected, Rejection, Upload,
    Violation, ViolationCode,
};
use base_web_media_type::MediaType;

/// One field as a format delivers it: its name, what its part says, and its content.
struct Submitted<'a> {
    name: &'a str,
    source: FieldSource<'a>,
    content: &'a [u8],
}

fn value<'a>(name: &'a str, content: &'a str) -> Submitted<'a> {
    Submitted {
        name,
        source: FieldSource::Value(Charset::Unspecified),
        content: content.as_bytes(),
    }
}

fn file<'a>(name: &'a str, media_type: Option<&'a MediaType>, content: &'a [u8]) -> Submitted<'a> {
    Submitted {
        name,
        source: FieldSource::File {
            filename: "name.bin",
            media_type,
        },
        content,
    }
}

/// Deliver `fields` to a form, storing each accepted upload with the next handle, counting from 1, and finish it.
fn submit<T: Item<u32> + Declaration>(
    fields: Vec<Submitted<'_>>,
) -> Result<Accepted<T, u32>, Rejected<u32>> {
    let mut form = Form::new(&T::DECLARATION);
    let mut handle = 0;
    for field in fields {
        if let Err(kind) = form.begin(field.name, field.source) {
            return form.finish(Some(crate::ContentError::new(kind, 0)));
        }
        if let (_, Some(kind)) = form.content(field.content) {
            return form.finish(Some(crate::ContentError::new(kind, 0)));
        }
        if form.end() {
            handle += 1;
            form.store(handle);
        }
    }
    form.finish(None)
}

fn accepted<T: Item<u32> + Declaration>(fields: Vec<Submitted<'_>>) -> T {
    submit::<T>(fields)
        .expect("the fields are accepted")
        .content
}

fn violations<T: Item<u32> + Declaration>(fields: Vec<Submitted<'_>>) -> Vec<String> {
    let rejected = submit::<T>(fields).expect_err("the fields are refused");
    let Rejection::Violations(violations) = rejected.reason else {
        panic!("expected violations, got {:?}", rejected.reason);
    };
    violations.iter().map(Violation::to_string).collect()
}

fn error<T: Item<u32> + Declaration>(fields: Vec<Submitted<'_>>) -> (ContentErrorKind, Vec<u32>) {
    let rejected = submit::<T>(fields).expect_err("the fields are refused");
    let Rejection::Error(error) = rejected.reason else {
        panic!("expected an error, got {:?}", rejected.reason);
    };
    (error.kind(), rejected.handles)
}

#[derive(Content, Debug, PartialEq)]
struct Address {
    city: String,
    postcode: Option<String>,
}

#[derive(Content, Debug, PartialEq)]
struct LineItem {
    name: String,
    #[content(count_limit = 2)]
    tags: Option<Vec<String>>,
}

#[derive(FieldValue, Debug, PartialEq)]
enum Role {
    #[content(rename = "admin")]
    Administrator,
    Editor,
}

#[derive(Content, Debug)]
struct Profile {
    name: String,
    #[content(rename = "e-mail")]
    email: Option<String>,
    age: Option<u8>,
    role: Option<Role>,
    #[content(count_limit = 3)]
    scores: Vec<i32>,
    address: Option<Address>,
    #[content(count_limit = 2)]
    items: Option<Vec<LineItem>>,
    #[content(accept("image/*"), size_limit = 8)]
    avatar: Option<Upload<u32>>,
    #[content(count_limit = 2)]
    attachments: Option<Vec<Upload<u32>>>,
    secret: Option<Vec<u8>>,
}

fn profile<'a>(mut extra: Vec<Submitted<'a>>) -> Vec<Submitted<'a>> {
    let mut fields = vec![value("name", "Ada"), value("scores[]", "1")];
    fields.append(&mut extra);
    fields
}

#[test]
fn builds_every_kind_of_field() {
    let png = MediaType::IMAGE_PNG;
    let profile = accepted::<Profile>(profile(vec![
        value("e-mail", "ada@example.com"),
        value("age", "36"),
        value("role", "admin"),
        value("scores[]", "-2"),
        value("address[city]", "London"),
        value("items[][name]", "first"),
        value("items[][tags][]", "a"),
        value("items[][name]", "second"),
        value("items[][tags][]", "b"),
        file("avatar", Some(&png), b"\x89PNG"),
        file("attachments[]", None, b"one"),
        file("attachments[]", Some(&png), b"two"),
        value("secret", "\u{0}bytes"),
    ]));
    assert_eq!(profile.name, "Ada");
    assert_eq!(profile.email.as_deref(), Some("ada@example.com"));
    assert_eq!(profile.age, Some(36));
    assert_eq!(profile.role, Some(Role::Administrator));
    assert_eq!(profile.scores, [1, -2]);
    assert_eq!(
        profile.address,
        Some(Address {
            city: "London".to_owned(),
            postcode: None
        })
    );
    assert_eq!(
        profile.items,
        Some(vec![
            LineItem {
                name: "first".to_owned(),
                tags: Some(vec!["a".to_owned()])
            },
            LineItem {
                name: "second".to_owned(),
                tags: Some(vec!["b".to_owned()])
            },
        ])
    );
    let avatar = profile.avatar.expect("an avatar");
    assert_eq!((avatar.size(), *avatar.handle()), (4, 1));
    assert_eq!(avatar.untrusted_filename(), "name.bin");
    assert_eq!(avatar.untrusted_media_type(), Some(&png));
    let attachments = profile.attachments.expect("attachments");
    assert_eq!(
        attachments
            .into_iter()
            .map(Upload::into_handle)
            .collect::<Vec<_>>(),
        [2, 3]
    );
    assert_eq!(profile.secret.as_deref(), Some(&b"\0bytes"[..]));
}

#[test]
fn omitted_optional_fields_are_none() {
    let profile = accepted::<Profile>(profile(Vec::new()));
    assert_eq!(
        (profile.email, profile.age, profile.role),
        (None, None, None)
    );
    assert!(profile.address.is_none() && profile.items.is_none());
    assert!(profile.avatar.is_none() && profile.attachments.is_none() && profile.secret.is_none());
}

#[test]
fn reports_every_violation_in_declaration_order() {
    let text = MediaType::TEXT_PLAIN;
    let found = violations::<Profile>(vec![
        file("secret", None, b"x"),
        value("age", "256"),
        value("role", "Administrator"),
        value("address[postcode]", "N1"),
        value("items[][name]", "first"),
        value("items[][tags][]", "a"),
        value("items[][tags][]", "b"),
        value("avatar", "text"),
        file("attachments[]", Some(&text), b"one"),
    ]);
    assert_eq!(
        found,
        [
            "name: required",
            "age: invalid_type",
            "role: invalid_type",
            "scores: required",
            "address[city]: required",
            "items[1][name]: required",
            "avatar: invalid_type",
            "secret: invalid_type",
        ]
    );
}

#[test]
fn a_refused_upload_is_reported_only_for_its_media_type() {
    #[derive(Content)]
    struct Pictures {
        #[content(accept("image/png", "image/jpeg"))]
        pictures: Vec<Upload<u32>>,
        #[content(accept("image/png"))]
        cover: Upload<u32>,
    }
    let (png, gif) = (MediaType::IMAGE_PNG, MediaType::IMAGE_GIF);
    let refused = submit::<Pictures>(vec![
        file("pictures[]", Some(&gif), b"gif"),
        file("pictures[]", None, b"none"),
        file("cover", Some(&png), b"png"),
        file("cover", Some(&gif), b"gif"),
    ])
    .expect_err("the uploads are refused");
    assert_eq!(
        refused.reason,
        Rejection::Violations(vec![
            Violation::new(
                path(&[
                    FieldPathSegment::Name("pictures"),
                    FieldPathSegment::Index(0)
                ]),
                ViolationCode::UnsupportedMediaType
            ),
            Violation::new(
                path(&[
                    FieldPathSegment::Name("pictures"),
                    FieldPathSegment::Index(1)
                ]),
                ViolationCode::UnsupportedMediaType
            ),
            Violation::new(
                path(&[FieldPathSegment::Name("cover")]),
                ViolationCode::UnsupportedMediaType
            ),
        ])
    );
    assert_eq!(
        refused.handles,
        [1],
        "the replaced cover's handle is returned"
    );
    let accepted = submit::<Pictures>(vec![
        file("pictures[]", Some(&MediaType::IMAGE_JPEG), b"jpeg"),
        file("cover", Some(&png), b"png"),
    ])
    .expect("the uploads are accepted");
    assert_eq!(accepted.content.pictures.len(), 1);
    assert_eq!(*accepted.content.cover.handle(), 2);
    assert!(accepted.discarded.is_empty());
}

fn path(segments: &[FieldPathSegment]) -> crate::FieldPath {
    let mut path = crate::FieldPath::default();
    path.extend(segments.iter().copied());
    path
}

#[test]
fn the_last_value_of_a_repeated_name_wins() {
    let png = MediaType::IMAGE_PNG;
    let accepted = submit::<Profile>(profile(vec![
        value("age", "x"),
        value("age", "7"),
        file("avatar", Some(&png), b"first"),
        file("avatar", Some(&png), b"second"),
        file("avatar", Some(&png), b"third"),
    ]))
    .expect("the last values are valid");
    assert_eq!(accepted.content.age, Some(7));
    assert_eq!(*accepted.content.avatar.expect("an avatar").handle(), 3);
    assert_eq!(accepted.discarded, [1, 2], "the replaced uploads' handles");

    let rejected = submit::<Profile>(profile(vec![
        file("avatar", Some(&png), b"first"),
        value("avatar", "replaced by a value"),
    ]))
    .expect_err("a value is not an upload");
    assert_eq!(rejected.handles, [1]);
}

#[test]
fn returns_every_handle_after_violations_or_an_error() {
    let png = MediaType::IMAGE_PNG;
    let uploads = || {
        vec![
            file("avatar", Some(&png), b"one"),
            file("avatar", Some(&png), b"two"),
            file("attachments[]", None, b"three"),
            file("items[][name]", None, b"a file for a scalar"),
        ]
    };
    let rejected = submit::<Profile>(profile(uploads())).expect_err("a file is not text");
    assert_eq!(rejected.handles, [1, 2, 3]);
    let mut fields = profile(uploads());
    fields.push(value("unknown", ""));
    assert_eq!(
        error::<Profile>(fields),
        (ContentErrorKind::UnknownName, vec![1, 2, 3])
    );
}

#[test]
fn matches_names_exactly() {
    for name in [
        "Name",
        "name[]",
        "scores",
        "scores[0]",
        "scores[][]",
        "address",
        "address[]",
        "address[city][]",
        "address[town]",
        "items[]",
        "items[0][name]",
        "items[][]",
        "items[][tags]",
        "",
        "[name]",
        "name]",
        "name[",
        "name[x",
        "na[me",
        "address[ci[ty]",
        "address[city]x",
        "address[]city",
    ] {
        assert_eq!(
            error::<Profile>(profile(vec![value(name, "")])).0,
            ContentErrorKind::UnknownName,
            "{name:?}"
        );
    }
}

#[test]
fn limits_the_depth_of_names() {
    #[derive(Content)]
    #[content(depth_limit = 3)]
    struct Shallow {
        a: Option<String>,
    }
    assert_eq!(
        accepted::<Shallow>(vec![value("a", "x")]).a.as_deref(),
        Some("x")
    );
    let (kind, _) = error::<Shallow>(vec![value("a[b][c]", "")]);
    assert_eq!(
        kind,
        ContentErrorKind::UnknownName,
        "three components are within the limit"
    );
    assert_eq!(
        error::<Shallow>(vec![value("a[b][c][d", "")]).0,
        ContentErrorKind::UnknownName,
        "an unfinished component is not counted"
    );
    assert_eq!(
        error::<Shallow>(vec![value("a[b][c][d]x", "")]).0,
        ContentErrorKind::UnknownName,
        "a malformed name is refused as one, however deep, as `base-web-url` reads it"
    );
    for name in ["a[b][c][d]", "a[b][c][d][e]", "a[][][]"] {
        assert_eq!(
            error::<Shallow>(vec![value(name, "")]).0,
            ContentErrorKind::DepthLimit,
            "{name}"
        );
    }
    let (kind, _) = error::<Profile>(profile(vec![value("a[b][c][d][e][f][g][h]", "")]));
    assert_eq!(kind, ContentErrorKind::UnknownName);
    let (kind, _) = error::<Profile>(profile(vec![value("a[b][c][d][e][f][g][h][i]", "")]));
    assert_eq!(kind, ContentErrorKind::DepthLimit, "the default limit is 8");
}

#[test]
fn limits_the_elements_of_arrays() {
    let scores = |count: usize| profile((1..count).map(|_| value("scores[]", "1")).collect());
    assert_eq!(accepted::<Profile>(scores(3)).scores.len(), 3);
    assert_eq!(error::<Profile>(scores(4)).0, ContentErrorKind::CountLimit);
    let items = |count: usize| profile((0..count).map(|_| value("items[][name]", "x")).collect());
    assert_eq!(
        accepted::<Profile>(items(2)).items.map(|items| items.len()),
        Some(2)
    );
    assert_eq!(error::<Profile>(items(3)).0, ContentErrorKind::CountLimit);
    let tags = |count: usize| {
        profile(
            [value("items[][name]", "x")]
                .into_iter()
                .chain((0..count).map(|_| value("items[][tags][]", "t")))
                .collect(),
        )
    };
    assert_eq!(
        violations::<Profile>(tags(2)),
        ["items[1][name]: required"],
        "a tag the element already has begins another element"
    );
    assert_eq!(error::<Profile>(tags(3)).0, ContentErrorKind::CountLimit);
}

#[test]
fn limits_the_size_of_fields_and_content() {
    #[derive(Content)]
    #[content(content_size_limit = 6)]
    struct Small {
        #[content(size_limit = 4)]
        a: Option<String>,
        b: Option<String>,
        #[content(size_limit = 2)]
        c: Option<Upload<u32>>,
    }
    assert_eq!(
        accepted::<Small>(vec![value("a", "1234")]).a.as_deref(),
        Some("1234")
    );
    assert_eq!(
        error::<Small>(vec![value("a", "12345")]).0,
        ContentErrorKind::FieldSizeLimit
    );
    let fields = || vec![value("a", "1234"), value("b", "12")];
    assert!(accepted::<Small>(fields()).b.is_some());
    let mut over = fields();
    over.push(value("b", "1"));
    assert_eq!(
        error::<Small>(over).0,
        ContentErrorKind::ContentSizeLimit,
        "repeated content counts"
    );
    let both = vec![value("b", "12"), value("a", "12345")];
    assert_eq!(
        error::<Small>(both).0,
        ContentErrorKind::FieldSizeLimit,
        "when both limits are reached, the field's is reported"
    );
    assert_eq!(
        error::<Small>(vec![file("c", None, b"123")]).0,
        ContentErrorKind::FieldSizeLimit
    );
    assert_eq!(
        error::<Small>(vec![value("c", "123")]).0,
        ContentErrorKind::FieldSizeLimit,
        "a refused field's content is limited too"
    );
    assert_eq!(
        accepted::<Small>(vec![file("c", None, b"12")])
            .c
            .map(|c| c.size()),
        Some(2)
    );
}

#[test]
fn refuses_text_in_other_character_sets() {
    #[derive(Content)]
    struct Text {
        text: Option<String>,
        number: Option<u32>,
        bytes: Option<Vec<u8>>,
    }
    let submitted = |name, charset, content: &'static [u8]| Submitted {
        name,
        source: FieldSource::Value(charset),
        content,
    };
    let text = accepted::<Text>(vec![
        submitted("text", Charset::Utf8, "café".as_bytes()),
        submitted("number", Charset::UsAscii, b"12"),
        submitted("bytes", Charset::Unsupported, b"\xff"),
    ]);
    assert_eq!(
        (text.text.as_deref(), text.number),
        (Some("café"), Some(12))
    );
    assert_eq!(
        text.bytes.as_deref(),
        Some(&b"\xff"[..]),
        "bytes are kept in any character set"
    );
    assert_eq!(
        violations::<Text>(vec![
            submitted("text", Charset::UsAscii, "café".as_bytes()),
            submitted("number", Charset::Unsupported, b"12"),
        ]),
        ["text: invalid_type", "number: invalid_type"]
    );
    assert_eq!(
        violations::<Text>(vec![submitted("text", Charset::Unspecified, b"\xff")]),
        ["text: invalid_type"]
    );
    assert_eq!(
        [
            Charset::from_parameter(None),
            Charset::from_parameter(Some("UTF-8")),
            Charset::from_parameter(Some("us-ascii")),
            Charset::from_parameter(Some("latin1"))
        ],
        [
            Charset::Unspecified,
            Charset::Utf8,
            Charset::UsAscii,
            Charset::Unsupported
        ]
    );
}

#[test]
fn a_required_group_needs_one_of_its_fields() {
    #[derive(Content)]
    struct Person {
        address: Address,
        #[content(count_limit = 2)]
        addresses: Vec<Address>,
    }
    assert_eq!(
        violations::<Person>(Vec::new()),
        ["address: required", "addresses: required"]
    );
    let person = accepted::<Person>(vec![
        value("address[city]", "Paris"),
        value("addresses[][city]", "Rome"),
        value("addresses[][postcode]", "00100"),
    ]);
    assert_eq!(person.address.city, "Paris");
    assert_eq!(
        person.addresses[0].postcode.as_deref(),
        Some("00100"),
        "a field the element lacks joins it"
    );
}

#[test]
fn an_empty_declaration_accepts_nothing() {
    #[derive(Content)]
    struct Empty {}
    let Empty {} = accepted::<Empty>(Vec::new());
    assert_eq!(
        error::<Empty>(vec![value("a", "")]).0,
        ContentErrorKind::UnknownName
    );
}

#[test]
fn a_nested_group_collects_its_fields() {
    let profile = accepted::<Profile>(profile(vec![
        value("address[city]", "Oslo"),
        value("address[postcode]", "0150"),
        value("address[city]", "Bergen"),
    ]));
    assert_eq!(
        profile.address,
        Some(Address {
            city: "Bergen".to_owned(),
            postcode: Some("0150".to_owned())
        }),
        "a nested group is not an array, so a repeated field replaces its value"
    );
}

#[test]
fn computes_the_depth_of_declarations() {
    use crate::declaration::depth;
    #[derive(Content)]
    struct Empty {}
    assert_eq!(depth(Empty::DECLARATION.group), 0);
    assert_eq!(depth(Address::DECLARATION.group), 1);
    assert_eq!(depth(LineItem::DECLARATION.group), 2, "`tags[]`");
    assert_eq!(depth(Profile::DECLARATION.group), 4, "`items[][tags][]`");
}

#[test]
fn errors_map_to_one_status_each() {
    for (kind, status) in [
        (ContentErrorKind::MissingMediaType, 415),
        (ContentErrorKind::UnsupportedMediaType, 415),
        (ContentErrorKind::UnacceptedFormat, 415),
        (ContentErrorKind::DepthLimit, 413),
        (ContentErrorKind::CountLimit, 413),
        (ContentErrorKind::FieldSizeLimit, 413),
        (ContentErrorKind::ContentSizeLimit, 413),
        (ContentErrorKind::UnknownName, 400),
        (ContentErrorKind::BodySizeLimit, 413),
        (ContentErrorKind::PreambleSizeLimit, 413),
        (ContentErrorKind::EpilogueSizeLimit, 413),
        (ContentErrorKind::HeaderSizeLimit, 413),
        (ContentErrorKind::HeaderCountLimit, 413),
        (ContentErrorKind::PartCountLimit, 413),
        (ContentErrorKind::InvalidBoundary, 400),
        (ContentErrorKind::MissingName, 400),
        (ContentErrorKind::InvalidPartHeader, 400),
        (ContentErrorKind::UnnamedFile, 400),
        (ContentErrorKind::UnsupportedCharset, 415),
        (ContentErrorKind::PairCountLimit, 413),
        (ContentErrorKind::UnexpectedEnd, 400),
        (ContentErrorKind::MalformedMultipart, 400),
        (ContentErrorKind::MalformedUrlEncoded, 400),
    ] {
        let error = crate::ContentError::new(kind, 42);
        assert_eq!(
            (error.kind(), error.offset(), error.status()),
            (kind, 42, status)
        );
        assert!(error.to_string().ends_with(" at byte 42"), "{error}");
    }
    assert_eq!(
        crate::ContentError::new(ContentErrorKind::UnknownName, 7).to_string(),
        "a name is not declared at byte 7"
    );
}

/// Formatting anything the parser returns shows none of what was submitted: values, filenames, media types, or undeclared names.
#[test]
fn formatting_never_shows_submitted_content() {
    const SECRETS: &[&str] = &[
        "hunter2",
        "passport-scan",
        "x-secret",
        "credit_card",
        "4111",
    ];
    let secret = MediaType::parse("application/x-secret").unwrap();
    let passport = |name| Submitted {
        name,
        source: FieldSource::File {
            filename: "passport-scan.png",
            media_type: Some(&secret),
        },
        content: b"hunter2",
    };
    let mut shown = Vec::new();
    let mut form = Form::<u32>::new(&Profile::DECLARATION);
    let head = form.begin(
        "attachments[]",
        FieldSource::File {
            filename: "passport-scan.png",
            media_type: Some(&secret),
        },
    );
    let head = head
        .expect("the upload is declared")
        .expect("the upload is accepted");
    assert_eq!(
        head.path().segments(),
        [
            FieldPathSegment::Name("attachments"),
            FieldPathSegment::Index(0)
        ]
    );
    assert_eq!(
        (head.untrusted_filename(), head.untrusted_media_type()),
        ("passport-scan.png", Some(&secret))
    );
    shown.push(format!("{head:?}"));

    let rejected = submit::<Profile>(profile(vec![
        value("e-mail", "hunter2"),
        passport("attachments[]"),
        passport("avatar"),
    ]))
    .expect_err("an avatar of an unaccepted media type");
    assert_eq!(
        rejected.reason.to_string(),
        "content has invalid fields: avatar: unsupported_media_type"
    );
    shown.push(format!(
        "{rejected:?} {} {:?}",
        rejected.reason, rejected.reason
    ));
    let accepted = submit::<Profile>(profile(vec![
        passport("attachments[]"),
        passport("attachments[]"),
    ]))
    .unwrap();
    let attachments = accepted.content.attachments.as_ref().expect("attachments");
    shown.push(format!("{accepted:?} {:?}", attachments[0]));
    let rejected = submit::<Profile>(profile(vec![value("credit_card", "4111")])).unwrap_err();
    shown.push(format!(
        "{rejected:?} {} {:?}",
        rejected.reason, rejected.reason
    ));
    let rejected = submit::<Profile>(vec![value("age", "hunter2")]).unwrap_err();
    let Rejection::Violations(violations) = &rejected.reason else {
        panic!("violations")
    };
    shown.push(format!(
        "{rejected:?} {} {:?} {:?}",
        rejected.reason,
        violations,
        violations[0].path()
    ));
    assert_eq!(
        rejected.reason.to_string(),
        "content has invalid fields: name: required, age: invalid_type, scores: required"
    );
    assert_eq!(violations[1].code(), ViolationCode::InvalidType);
    assert_eq!(
        ViolationCode::UnsupportedMediaType.to_string(),
        "unsupported_media_type"
    );
    for text in &shown {
        for secret in SECRETS {
            assert!(!text.contains(secret), "{secret:?} in {text}");
        }
    }
    assert!(
        shown[0].contains("attachments[0]") && shown[0].contains("<17 bytes>"),
        "{}",
        shown[0]
    );
}

#[test]
fn a_field_without_a_value_is_an_invalid_type() {
    #[derive(Content)]
    #[expect(dead_code, reason = "only the violations are checked")]
    struct Everything {
        text: String,
        tags: Vec<String>,
        file: Upload<u32>,
        address: Address,
    }
    let absent = |name| Submitted {
        name,
        source: FieldSource::Absent,
        content: b"",
    };
    assert_eq!(
        violations::<Everything>(vec![
            absent("text"),
            absent("tags[]"),
            absent("file"),
            absent("address[city]"),
        ]),
        [
            "text: invalid_type",
            "tags[0]: invalid_type",
            "file: invalid_type",
            "address[city]: invalid_type",
        ],
        "a required field reports only its type, not that it is required"
    );
}

#[test]
fn uploads_can_be_made_for_tests() {
    let upload = Upload::new("a.png", Some(MediaType::IMAGE_PNG), 3, 7u32);
    assert_eq!(
        (
            upload.untrusted_filename(),
            upload.untrusted_media_type(),
            upload.size(),
            *upload.handle()
        ),
        ("a.png", Some(&MediaType::IMAGE_PNG), 3, 7)
    );
}
