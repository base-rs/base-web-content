// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::Signup;
use crate::support::{
    Handle, MEDIA_TYPE, Seen, body, field, file, part, run_parser, split_invariant,
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
        file("documents[]", "", None, b""),
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
            upload("documents[0]", "", None),
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
        "an empty filename makes an upload, and a filename is kept as it was sent"
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
