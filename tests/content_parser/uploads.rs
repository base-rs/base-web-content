// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::Signup;
use crate::support::{
    Handle, MEDIA_TYPE, Seen, body, field, file, part, refused_at, run_parser, split_invariant,
};
use base_web_content::{Content, ContentErrorKind, ContentParser, Rejection, Upload};
use base_web_media_type::MediaType;
use base_web_multipart::ParameterRules;

fn upload(path: &str, filename: &str, media_type: Option<&str>) -> Seen {
    Seen::Upload {
        path: path.to_owned(),
        filename: filename.to_owned(),
        media_type: media_type.map(str::to_owned),
    }
}

#[test]
fn streams_accepted_uploads_to_the_caller() {
    let body = body(&[
        field("name", "Ada"),
        file("avatar", "me.png", Some("image/png"), b"\x89PNG\r\n\x1a\n"),
        file("documents[]", "empty.txt", None, b""),
        file(
            "documents[]",
            "c:\\notes.txt",
            Some("text/plain; charset=utf-8"),
            b"notes",
        ),
    ]);
    let run = split_invariant::<Signup>(&body);
    assert_eq!(
        run.seen,
        [
            upload("avatar", "me.png", Some("image/png")),
            Seen::Content(b"\x89PNG\r\n\x1a\n".to_vec()),
            Seen::UploadEnd,
            upload("documents[0]", "empty.txt", None),
            Seen::UploadEnd,
            upload(
                "documents[1]",
                "c:\\notes.txt",
                Some("text/plain; charset=utf-8")
            ),
            Seen::Content(b"notes".to_vec()),
            Seen::UploadEnd,
            Seen::End,
        ],
        "an empty file is an upload, and a filename is kept as it was sent"
    );
    let stored = run.stored.clone();
    let accepted = run.accepted();
    let avatar = accepted.content.avatar.expect("an avatar");
    assert_eq!((*avatar.handle(), avatar.size()), (1, 8));
    assert_eq!(stored[0], b"\x89PNG\r\n\x1a\n");
    assert_eq!(avatar.untrusted_filename(), "me.png");
    assert_eq!(avatar.untrusted_media_type(), Some(&MediaType::IMAGE_PNG));
    let documents = accepted.content.documents.expect("documents");
    assert_eq!(
        documents
            .iter()
            .map(|document| *document.handle())
            .collect::<Vec<_>>(),
        [2, 3]
    );
    assert_eq!(
        (documents[0].size(), documents[0].untrusted_media_type()),
        (0, None)
    );
}

/// The part a browser sends for a file input with no file chosen: an empty filename, no content, and usually `application/octet-stream`.
fn no_file(name: &str) -> Vec<u8> {
    file(name, "", Some("application/octet-stream"), b"")
}

#[test]
fn a_file_input_with_no_file_chosen_is_absent() {
    let run = split_invariant::<Signup>(&body(&[
        field("name", "Ada"),
        no_file("avatar"),
        no_file("documents[]"),
        part(
            &[(
                "Content-Disposition",
                "form-data; name=\"documents[]\"; filename=\"\"",
            )],
            b"",
        ),
    ]));
    assert_eq!(
        run.seen,
        [Seen::End],
        "no upload begins, and `accept` does not refuse it"
    );
    let accepted = run.accepted();
    assert!(accepted.content.avatar.is_none());
    assert!(accepted.content.documents.is_none(), "no element is added");
    assert!(accepted.discarded.is_empty());
}

#[test]
fn no_file_chosen_neither_replaces_an_upload_nor_adds_an_element() {
    let body = body(&[
        field("name", "Ada"),
        file("avatar", "me.png", Some("image/png"), b"png"),
        no_file("avatar"),
        no_file("documents[]"),
        file("documents[]", "a.txt", None, b"a"),
        no_file("documents[]"),
        no_file("documents[]"),
        file("documents[]", "b.txt", None, b"b"),
        no_file("documents[]"),
    ]);
    let run = split_invariant::<Signup>(&body);
    assert_eq!(
        run.seen
            .iter()
            .filter(|seen| matches!(seen, Seen::Upload { .. }))
            .count(),
        3
    );
    let accepted = run.accepted();
    assert_eq!(*accepted.content.avatar.unwrap().handle(), 1);
    let documents = accepted.content.documents.unwrap();
    assert_eq!(
        documents
            .iter()
            .map(|document| *document.handle())
            .collect::<Vec<_>>(),
        [2, 3],
        "parts with no file chosen do not count toward the count limit of 2"
    );
    assert!(accepted.discarded.is_empty());
}

#[test]
fn no_file_chosen_does_not_replace_a_refused_value() {
    for (refused, code) in [
        (field("avatar", "me.png"), "invalid_type"),
        (
            file("avatar", "me.gif", Some("image/gif"), b"gif"),
            "unsupported_media_type",
        ),
    ] {
        let run =
            split_invariant::<Signup>(&body(&[field("name", "Ada"), refused, no_file("avatar")]));
        assert_eq!(run.violations(), [format!("avatar: {code}")]);
    }
}

#[test]
fn a_required_upload_with_no_file_chosen_is_required() {
    #[derive(Content, Debug)]
    struct Application {
        #[content(rename = "resume", accept("application/pdf"))]
        _resume: Upload<Handle>,
        #[content(rename = "photos", accept("image/*"))]
        _photos: Vec<Upload<Handle>>,
    }
    let body = body(&[no_file("resume"), no_file("photos[]"), no_file("photos[]")]);
    let run = split_invariant::<Application>(&body);
    assert_eq!(run.seen, [Seen::End]);
    assert_eq!(run.violations(), ["resume: required", "photos: required"]);
}

#[derive(Content, Debug)]
struct Photo {
    caption: String,
    #[content(accept("image/*"))]
    image: Option<Upload<Handle>>,
    #[content(accept("image/*"))]
    variants: Option<Vec<Upload<Handle>>>,
}

#[derive(Content, Debug)]
struct Gallery {
    #[content(count_limit = 2)]
    photos: Vec<Photo>,
}

/// Each photo's caption, its image's handle, and its variants' handles.
fn photos(run: crate::support::Run<Gallery>) -> Vec<(String, Option<Handle>, Option<Vec<Handle>>)> {
    let handles =
        |uploads: &Vec<Upload<Handle>>| uploads.iter().map(|upload| *upload.handle()).collect();
    run.accepted()
        .content
        .photos
        .iter()
        .map(|photo| {
            (
                photo.caption.clone(),
                photo.image.as_ref().map(|image| *image.handle()),
                photo.variants.as_ref().map(handles),
            )
        })
        .collect()
}

#[test]
fn no_file_chosen_ends_an_element_of_an_array_of_groups() {
    // Two rows of a form, in the order a browser submits them, the first with no file chosen.
    let image_first = body(&[
        no_file("photos[][image]"),
        field("photos[][caption]", "first"),
        file("photos[][image]", "2.png", Some("image/png"), b"png"),
        field("photos[][caption]", "second"),
    ]);
    let run = split_invariant::<Gallery>(&image_first);
    assert_eq!(
        run.seen[0],
        upload("photos[1][image]", "2.png", Some("image/png"))
    );
    assert_eq!(
        photos(run),
        [
            ("first".to_owned(), None, None),
            ("second".to_owned(), Some(1), None)
        ],
        "the file belongs to the second row"
    );
    let caption_first = body(&[
        field("photos[][caption]", "first"),
        no_file("photos[][image]"),
        field("photos[][caption]", "second"),
        file("photos[][image]", "2.png", Some("image/png"), b"png"),
    ]);
    assert_eq!(
        photos(split_invariant::<Gallery>(&caption_first)),
        [
            ("first".to_owned(), None, None),
            ("second".to_owned(), Some(1), None)
        ]
    );
    let variants = body(&[
        no_file("photos[][variants][]"),
        field("photos[][caption]", "first"),
        file("photos[][variants][]", "2.png", Some("image/png"), b"png"),
        field("photos[][caption]", "second"),
    ]);
    assert_eq!(
        photos(split_invariant::<Gallery>(&variants)),
        [
            ("first".to_owned(), None, None),
            ("second".to_owned(), None, Some(vec![1]))
        ],
        "an array with only no file chosen is absent, and ends the element as any field does"
    );
}

#[test]
fn no_file_chosen_in_a_nested_group_is_reported_at_the_upload() {
    #[derive(Content, Debug)]
    struct Profile {
        #[content(rename = "avatar")]
        _avatar: Upload<Handle>,
    }
    #[derive(Content, Debug)]
    struct Account {
        #[content(rename = "profile")]
        _profile: Option<Profile>,
    }
    let run = split_invariant::<Account>(&body(&[no_file("profile[avatar]")]));
    assert_eq!(run.violations(), ["profile[avatar]: required"]);
}

#[test]
fn no_file_chosen_counts_toward_the_part_count_limit() {
    #[derive(Content, Debug)]
    #[content(part_count_limit = 2)]
    struct Avatars {
        #[content(rename = "avatars")]
        _avatars: Option<Vec<Upload<Handle>>>,
    }
    let body = body(&[
        no_file("avatars[]"),
        no_file("avatars[]"),
        no_file("avatars[]"),
    ]);
    let error = split_invariant::<Avatars>(&body).error();
    assert_eq!(error.kind(), ContentErrorKind::PartCountLimit);
}

#[test]
fn no_file_chosen_with_content_is_refused() {
    let body = body(&[
        field("name", "Ada"),
        file("avatar", "me.png", Some("image/png"), b"png"),
        file("avatar", "", Some("image/png"), b"hidden"),
    ]);
    let run = split_invariant::<Signup>(&body);
    assert_eq!(
        run.seen,
        [
            upload("avatar", "me.png", Some("image/png")),
            Seen::Content(b"png".to_vec()),
            Seen::UploadEnd,
        ],
        "none of its content is reported"
    );
    let rejected = run.rejected();
    let Rejection::Error(error) = rejected.reason else {
        panic!("an error")
    };
    let start = body
        .windows(6)
        .position(|window| window == b"hidden")
        .unwrap();
    assert_eq!(
        (error.kind(), error.offset(), error.status()),
        (ContentErrorKind::EmptyFilenameContent, start as u64, 400),
        "refused at its first byte of content"
    );
    assert_eq!(
        rejected.handles,
        [1],
        "the earlier upload's handle is returned"
    );
    let (_, fed) = refused_at::<Signup>(&body);
    assert_eq!(fed, start + 1, "refused as soon as its content arrives");
}

#[test]
fn no_file_chosen_with_content_before_a_bare_line_break_is_refused() {
    let mut body = field("name", "Ada");
    body.extend_from_slice(&part(
        &[(
            "Content-Disposition",
            "form-data; name=\"avatar\"; filename=\"\"",
        )],
        b"x\n--XyZ--\r\n",
    ));
    let error = split_invariant::<Signup>(&body).error();
    let start = body
        .windows(3)
        .position(|window| window == b"x\n-")
        .unwrap();
    assert_eq!(
        (error.kind(), error.offset()),
        (ContentErrorKind::EmptyFilenameContent, start as u64),
        "its content is refused before the malformed boundary after it"
    );
}

#[test]
fn no_file_chosen_with_content_that_could_begin_a_boundary_is_refused() {
    for content in [&b"\r\n-x"[..], b"-", b"\r", b"\r\n--Xy"] {
        let body = body(&[
            field("name", "Ada"),
            file("avatar", "", Some("application/octet-stream"), content),
        ]);
        let error = split_invariant::<Signup>(&body).error();
        let start = body.len() - b"\r\n--XyZ--\r\n".len() - content.len();
        assert_eq!(
            (error.kind(), error.offset()),
            (ContentErrorKind::EmptyFilenameContent, start as u64),
            "{content:?}"
        );
    }
}

#[test]
fn a_file_with_an_empty_filename_is_still_a_file_for_a_scalar() {
    let run = split_invariant::<Signup>(&body(&[field("name", "Ada"), no_file("e-mail")]));
    assert_eq!(run.violations(), ["e-mail: invalid_type"]);
}

#[test]
fn an_extended_filename_takes_the_place_of_an_empty_one() {
    let body = body(&[
        field("name", "Ada"),
        part(
            &[
                (
                    "Content-Disposition",
                    "form-data; name=\"avatar\"; filename=\"\"; filename*=utf-8''a.png",
                ),
                ("Content-Type", "image/png"),
            ],
            b"png",
        ),
    ]);
    let media_type = MediaType::parse(MEDIA_TYPE).unwrap();
    let parser = ContentParser::<Signup, Handle>::new(Some(&media_type))
        .unwrap()
        .with_parameter_rules(ParameterRules::Mime);
    let run = run_parser(parser, &[&body], b"");
    assert_eq!(run.seen[0], upload("avatar", "a.png", Some("image/png")));
    assert_eq!(
        split_invariant::<Signup>(&body).error().kind(),
        ContentErrorKind::InvalidPartHeader,
        "form data refuses the extended filename"
    );
}

#[test]
fn an_extended_filename_that_decodes_empty_chose_no_file() {
    let body = body(&[
        field("name", "Ada"),
        part(
            &[(
                "Content-Disposition",
                "form-data; name=\"avatar\"; filename*=utf-8''",
            )],
            b"",
        ),
    ]);
    let media_type = MediaType::parse(MEDIA_TYPE).unwrap();
    let parser = ContentParser::<Signup, Handle>::new(Some(&media_type))
        .unwrap()
        .with_parameter_rules(ParameterRules::Mime);
    let run = run_parser(parser, &[&body], b"");
    assert_eq!(run.seen, [Seen::End]);
    assert!(run.accepted().content.avatar.is_none());
}

#[test]
fn checks_accept_against_the_declared_media_type_only() {
    for (part, reason) in [
        (
            file("avatar", "me.png", None, b"png"),
            "without a Content-Type",
        ),
        (
            file("avatar", "me.png", Some("application/octet-stream"), b"png"),
            "a filename does not make it acceptable",
        ),
        (
            file("avatar", "me.png", Some("image/gif"), b"gif"),
            "another image type",
        ),
    ] {
        let run = split_invariant::<Signup>(&body(&[field("name", "Ada"), part]));
        assert_eq!(
            run.seen,
            [Seen::End],
            "{reason}: its content is not reported"
        );
        assert_eq!(
            run.violations(),
            ["avatar: unsupported_media_type"],
            "{reason}"
        );
    }
    let jpeg = file("avatar", "me", Some("IMAGE/JPEG; quality=high"), b"jpeg");
    let run = split_invariant::<Signup>(&body(&[field("name", "Ada"), jpeg]));
    assert_eq!(
        *run.accepted().content.avatar.unwrap().handle(),
        1,
        "media types match without case or parameters"
    );
}

#[test]
fn a_file_and_a_value_are_not_interchangeable() {
    let run = split_invariant::<Signup>(&body(&[
        field("name", "Ada"),
        file(
            "e-mail",
            "address.txt",
            Some("text/plain"),
            b"ada@example.com",
        ),
        field("avatar", "me.png"),
    ]));
    assert_eq!(run.seen, [Seen::End]);
    assert_eq!(
        run.violations(),
        ["e-mail: invalid_type", "avatar: invalid_type"]
    );
}

#[test]
fn returns_replaced_uploads_as_discarded() {
    let body = body(&[
        field("name", "Ada"),
        file("avatar", "1.png", Some("image/png"), b"one"),
        file("avatar", "2.png", Some("image/png"), b"two"),
        file("avatar", "3.png", Some("image/png"), b"three"),
    ]);
    let accepted = split_invariant::<Signup>(&body).accepted();
    assert_eq!(*accepted.content.avatar.unwrap().handle(), 3);
    assert_eq!(accepted.discarded, [1, 2]);
}

#[test]
fn returns_every_handle_after_violations() {
    let body = body(&[
        file("avatar", "1.png", Some("image/png"), b"one"),
        file("avatar", "2.png", Some("image/png"), b"two"),
        file("documents[]", "3.txt", None, b"three"),
        field("age", "old"),
    ]);
    let rejected = split_invariant::<Signup>(&body).rejected();
    assert_eq!(
        rejected.reason.to_string(),
        "content has invalid fields: name: required, age: invalid_type"
    );
    assert_eq!(rejected.handles, [1, 2, 3]);
}

#[test]
fn returns_every_handle_after_an_error() {
    let body = body(&[
        file("avatar", "1.png", Some("image/png"), b"one"),
        file("avatar", "2.png", Some("image/png"), b"two"),
        file("documents[]", "3.txt", None, b"three"),
        file("documents[]", "4.txt", None, b"far too long for its limit"),
    ]);
    let run = split_invariant::<Signup>(&body);
    assert_eq!(
        run.seen
            .iter()
            .filter(|seen| **seen == Seen::UploadEnd)
            .count(),
        3,
        "the fourth upload never ends"
    );
    assert_eq!(
        run.seen.last(),
        Some(&Seen::Content(b"far too long for".to_vec())),
        "content within the limit is reported"
    );
    let rejected = run.rejected();
    let Rejection::Error(error) = rejected.reason else {
        panic!("an error")
    };
    assert_eq!(error.kind(), ContentErrorKind::FieldSizeLimit);
    assert_eq!(rejected.handles, [1, 2, 3]);
}

#[test]
fn reads_names_and_filenames_under_the_parameter_rules() {
    let extended = part(
        &[(
            "Content-Disposition",
            "form-data; name=\"documents[]\"; filename=\"rates.txt\"; filename*=utf-8''%e2%82%ac%20rates.txt",
        )],
        b"rates",
    );
    let body = body(&[field("name", "Ada"), extended]);
    let error = split_invariant::<Signup>(&body).error();
    assert_eq!(
        error.kind(),
        ContentErrorKind::InvalidPartHeader,
        "form data refuses extended parameters"
    );
    let media_type = MediaType::parse(MEDIA_TYPE).unwrap();
    let parser = ContentParser::<Signup, Handle>::new(Some(&media_type))
        .unwrap()
        .with_parameter_rules(ParameterRules::Mime);
    let run = run_parser(parser, &[&body], b"");
    assert_eq!(
        run.seen[0],
        upload("documents[0]", "€ rates.txt", None),
        "MIME rules decode them"
    );
    let undecodable = part(
        &[(
            "Content-Disposition",
            "form-data; name=\"documents[]\"; filename*=utf-8''%zz",
        )],
        b"rates",
    );
    let parser = ContentParser::<Signup, Handle>::new(Some(&media_type))
        .unwrap()
        .with_parameter_rules(ParameterRules::Mime);
    let run = run_parser(parser, &[], &crate::support::body(&[undecodable]));
    assert_eq!(
        run.error().kind(),
        ContentErrorKind::InvalidPartHeader,
        "a filename that cannot be decoded"
    );
}

#[test]
#[should_panic(expected = "its handle must be given to `store` before parsing on")]
fn parsing_on_without_storing_an_upload_panics() {
    #[derive(Content)]
    struct Avatar {
        _avatar: Upload<Handle>,
    }
    let media_type = MediaType::parse(MEDIA_TYPE).unwrap();
    let mut parser = ContentParser::<Avatar, Handle>::new(Some(&media_type)).unwrap();
    let body = body(&[file("_avatar", "a.png", None, b"")]);
    let mut input = &body[..];
    while parser
        .parse(input)
        .map(|parsed| input = &input[parsed.consumed..])
        .is_ok()
    {}
}

#[test]
#[should_panic(expected = "an upload ended and waits for its handle")]
fn storing_without_an_upload_panics() {
    let media_type = MediaType::parse(MEDIA_TYPE).unwrap();
    let mut parser = ContentParser::<Signup, Handle>::new(Some(&media_type)).unwrap();
    parser.store(1);
}

#[test]
fn finishing_before_storing_refuses_the_body() {
    #[derive(Content, Debug)]
    struct Avatar {
        _avatar: Upload<Handle>,
    }
    let media_type = MediaType::parse(MEDIA_TYPE).unwrap();
    let mut parser = ContentParser::<Avatar, Handle>::new(Some(&media_type)).unwrap();
    let body = body(&[file("_avatar", "a.png", None, b"a")]);
    let mut input = &body[..];
    loop {
        let parsed = parser.parse(input).unwrap();
        input = &input[parsed.consumed..];
        if parsed.event == base_web_content::ContentEvent::UploadEnd {
            break;
        }
    }
    let rejected = parser.finish().unwrap_err();
    let Rejection::Error(error) = rejected.reason else {
        panic!("an error")
    };
    assert_eq!(error.kind(), ContentErrorKind::UnexpectedEnd);
    assert!(rejected.handles.is_empty());
}

#[test]
fn a_parser_can_be_held_across_threads() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ContentParser<Signup, Handle>>();
    assert_send_sync::<base_web_content::Accepted<Signup, Handle>>();
    assert_send_sync::<base_web_content::Rejected<Handle>>();
}

#[test]
fn a_filename_is_given_as_it_was_sent_with_any_character() {
    let filename = "invoice\t\u{202e}fdp.exe";
    let run = split_invariant::<Signup>(&body(&[
        field("name", "Ada"),
        file("documents[]", filename, None, b"x"),
    ]));
    assert_eq!(run.seen[0], upload("documents[0]", filename, None));
}
