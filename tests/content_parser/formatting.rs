// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::Signup;
use crate::support::{Handle, MEDIA_TYPE, body, field, file, part};
use base_web_content::{ContentEvent, ContentParser};
use base_web_media_type::MediaType;

/// Distinctive submitted values, filenames, media types, and undeclared names, none of which formatting may show.
const SUBMITTED: &[&str] = &[
    "hunter2",
    "passport-scan",
    "x-secret",
    "credit_card",
    "4111",
    "Ada Lovelace",
];

/// Parse `body`, formatting every event, error, and result with `Display` and `Debug`.
fn formatted(body: &[u8]) -> Vec<String> {
    let media_type = MediaType::parse(MEDIA_TYPE).unwrap();
    let mut parser = ContentParser::<Signup, Handle>::new(Some(&media_type)).unwrap();
    let mut shown = vec![format!("{parser:?}")];
    let (mut input, mut handle) = (body, 0);
    loop {
        match parser.parse(input) {
            Ok(parsed) => {
                shown.push(format!("{parsed:?}"));
                input = &input[parsed.consumed..];
                match parsed.event {
                    ContentEvent::UploadEnd => {
                        handle += 1;
                        parser.store(handle);
                    }
                    ContentEvent::End | ContentEvent::Incomplete => break,
                    ContentEvent::Upload(_) | ContentEvent::UploadContent(_) => {}
                }
            }
            Err(error) => {
                shown.push(format!("{error} {error:?}"));
                break;
            }
        }
    }
    shown.push(format!("{parser:?}"));
    match parser.finish() {
        Ok(accepted) => {
            shown.push(format!("{accepted:?}"));
            shown.extend(
                accepted
                    .content
                    .avatar
                    .iter()
                    .map(|avatar| format!("{avatar:?}")),
            );
            shown.extend(
                accepted
                    .content
                    .documents
                    .iter()
                    .flatten()
                    .map(|document| format!("{document:?}")),
            );
        }
        Err(rejected) => shown.push(format!(
            "{rejected:?} {} {:?}",
            rejected.reason, rejected.reason
        )),
    }
    shown
}

#[track_caller]
fn assert_hidden(shown: &[String]) {
    for text in shown {
        for submitted in SUBMITTED {
            assert!(!text.contains(submitted), "{submitted:?} in {text}");
        }
    }
}

#[test]
fn never_shows_submitted_content() {
    let secret = part(
        &[
            (
                "Content-Disposition",
                "form-data; name=\"documents[]\"; filename=\"passport-scan.png\"",
            ),
            ("Content-Type", "application/x-secret"),
        ],
        b"hunter2",
    );
    let accepted = formatted(&body(&[
        field("name", "Ada Lovelace"),
        field("e-mail", "hunter2"),
        secret.clone(),
        secret.clone(),
    ]));
    assert!(
        accepted
            .iter()
            .any(|text| text.contains("UploadContent(<7 bytes>)")),
        "{accepted:?}"
    );
    assert!(
        accepted.iter().any(|text| text.contains("documents[1]")),
        "{accepted:?}"
    );
    assert!(
        accepted.iter().any(|text| text.starts_with("Accepted")),
        "{accepted:?}"
    );
    assert_hidden(&accepted);
    let refused = formatted(&body(&[
        secret.clone(),
        file(
            "avatar",
            "passport-scan.png",
            Some("application/x-secret"),
            b"hunter2",
        ),
        field("age", "hunter2"),
    ]));
    assert!(
        refused
            .iter()
            .any(|text| text.contains("avatar: unsupported_media_type")),
        "{refused:?}"
    );
    assert_hidden(&refused);
    let undeclared = formatted(&body(&[secret, field("credit_card", "4111")]));
    assert!(
        undeclared
            .iter()
            .any(|text| text.contains("a name is not declared")),
        "{undeclared:?}"
    );
    assert_hidden(&undeclared);
    assert_eq!(format!("{:?}", ContentEvent::Incomplete), "Incomplete");
}
