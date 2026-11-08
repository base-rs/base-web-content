// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::support::{Seen, body, field, file, part, split_invariant};
use crate::{Address, Line, Plan, Signup};

#[test]
fn parses_every_kind_of_field_however_the_body_is_split() {
    let body = body(&[
        field("name", "Ada"),
        field("e-mail", "ada@example.com"),
        field("age", "36"),
        field("plan", "pro"),
        field("tags[]", "maths"),
        field("tags[]", "engines"),
        field("address[city]", "London"),
        field("lines[][sku]", "A1"),
        field("lines[][quantity]", "2"),
        field("lines[][sku]", "B2"),
        field("secret", "\u{0}\r\n--XyY"),
    ]);
    let run = split_invariant::<Signup>(&body);
    assert_eq!(
        run.seen,
        [Seen::End],
        "fields other than uploads are not reported"
    );
    let signup = run.accepted().content;
    assert_eq!(signup.name, "Ada");
    assert_eq!(signup.email.as_deref(), Some("ada@example.com"));
    assert_eq!((signup.age, signup.plan), (Some(36), Some(Plan::Pro)));
    assert_eq!(
        signup.tags,
        Some(vec!["maths".to_owned(), "engines".to_owned()])
    );
    assert_eq!(
        signup.address,
        Some(Address {
            city: "London".to_owned(),
            postcode: None
        })
    );
    assert_eq!(
        signup.lines,
        Some(vec![
            Line {
                sku: "A1".to_owned(),
                quantity: Some(2)
            },
            Line {
                sku: "B2".to_owned(),
                quantity: None
            },
        ])
    );
    assert_eq!(signup.secret.as_deref(), Some(&b"\0\r\n--XyY"[..]));
}

#[test]
fn reports_every_violation_in_declaration_order() {
    let body = body(&[
        field("plan", "enterprise"),
        field("age", "-1"),
        field("tags[]", "ok"),
        file("tags[]", "tag.txt", Some("text/plain"), b"a file"),
        field("address[postcode]", "N1"),
        field("avatar", "not a file"),
    ]);
    let run = split_invariant::<Signup>(&body);
    assert_eq!(run.seen, [Seen::End], "a file for a scalar is not reported");
    assert_eq!(
        run.violations(),
        [
            "name: required",
            "age: invalid_type",
            "plan: invalid_type",
            "tags[1]: invalid_type",
            "address[city]: required",
            "avatar: invalid_type",
        ]
    );
}

#[test]
fn refuses_text_in_other_character_sets() {
    let text = |charset: &str, value: &[u8]| {
        part(
            &[
                ("Content-Disposition", "form-data; name=\"e-mail\""),
                ("Content-Type", &format!("text/plain; charset={charset}")),
            ],
            value,
        )
    };
    let accepted = |part| {
        split_invariant::<Signup>(&body(&[field("name", "Ada"), part]))
            .accepted()
            .content
            .email
    };
    assert_eq!(
        accepted(text("UTF-8", "é".as_bytes())).as_deref(),
        Some("é")
    );
    assert_eq!(
        accepted(text("us-ascii", b"plain")).as_deref(),
        Some("plain")
    );
    let refused =
        |part| split_invariant::<Signup>(&body(&[field("name", "Ada"), part])).violations();
    assert_eq!(
        refused(text("iso-8859-1", b"caf\xe9")),
        ["e-mail: invalid_type"]
    );
    assert_eq!(
        refused(text("us-ascii", "é".as_bytes())),
        ["e-mail: invalid_type"]
    );
    assert_eq!(refused(text("utf-8", b"caf\xe9")), ["e-mail: invalid_type"]);
    let bytes = part(
        &[
            ("Content-Disposition", "form-data; name=\"secret\""),
            ("Content-Type", "text/plain; charset=iso-8859-1"),
        ],
        b"caf\xe9",
    );
    let secret = split_invariant::<Signup>(&body(&[field("name", "Ada"), bytes]))
        .accepted()
        .content
        .secret;
    assert_eq!(
        secret.as_deref(),
        Some(&b"caf\xe9"[..]),
        "bytes are kept in any character set"
    );
}

#[test]
fn the_last_value_of_a_repeated_name_wins() {
    let body = body(&[
        field("name", "Ada"),
        field("age", "x"),
        field("name", "Grace"),
        field("age", "7"),
    ]);
    let signup = split_invariant::<Signup>(&body).accepted().content;
    assert_eq!((signup.name.as_str(), signup.age), ("Grace", Some(7)));
}

#[test]
fn an_empty_body_has_no_fields() {
    let run = split_invariant::<Signup>(&body(&[]));
    assert_eq!(run.seen, [Seen::End]);
    assert_eq!(run.violations(), ["name: required"]);
}

#[test]
fn matches_names_exactly() {
    for name in [
        "tags",
        "tags[0]",
        "address",
        "lines[0][sku]",
        "lines[][]",
        "Name",
        "name[]",
        "e-mail[]",
    ] {
        let body = body(&[field("name", "Ada"), field(name, "x")]);
        let error = split_invariant::<Signup>(&body).error();
        assert_eq!(
            (error.kind(), error.offset()),
            (
                base_web_content::ContentErrorKind::UnknownName,
                (field("name", "Ada").len() + "--XyZ\r\n".len()) as u64
            ),
            "{name}: refused at its header section, after the first part"
        );
    }
}
