// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::support::{Handle, Seen, run};
use crate::{Address, Line, Plan, Signup};
use base_web_multipart::{Boundary, ContentDisposition, MultipartWriter};

/// A field to write: its name, its filename and media type if it is a file, and its content.
type Written<'a> = (&'a str, Option<(&'a str, &'a str)>, &'a [u8]);

/// Write a form with `base-web-multipart`'s writer, as a client would, with a generated boundary.
fn write(fields: &[Written<'_>]) -> (String, Vec<u8>) {
    let boundary = Boundary::generate();
    let media_type = boundary.form_data_media_type().to_string();
    let mut writer = MultipartWriter::new(boundary);
    let mut body = Vec::new();
    for (name, file, content) in fields {
        let mut disposition = ContentDisposition::form_data(name).unwrap();
        let mut headers = Vec::new();
        if let Some((filename, media_type)) = file {
            disposition = disposition.with_parameter("filename", filename).unwrap();
            headers.push(("Content-Type".to_owned(), media_type.to_string()));
        }
        headers.insert(
            0,
            ("Content-Disposition".to_owned(), disposition.to_string()),
        );
        writer
            .start_part(
                headers.iter().map(|(name, value)| (name, value.as_bytes())),
                &mut body,
            )
            .unwrap();
        writer.write_content(content, &mut body).unwrap();
    }
    writer.finish(&mut body);
    (media_type, body)
}

#[test]
fn parses_what_the_writer_writes() {
    let png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
    let (media_type, body) = write(&[
        ("name", None, "Zoë \"Z\" O'Neil\r\nthe second".as_bytes()),
        ("plan", None, b"free"),
        ("tags[]", None, b"a"),
        ("tags[]", None, b"b"),
        ("address[city]", None, b"Z\xc3\xbcrich"),
        ("address[postcode]", None, b"8001"),
        ("lines[][sku]", None, b"X"),
        ("lines[][sku]", None, b"Y"),
        ("lines[][quantity]", None, b"+3"),
        ("avatar", Some(("photo \"1\".png", "image/png")), png),
        ("documents[]", Some(("a.txt", "text/plain")), b"\r\n--"),
    ]);
    let chunks: Vec<&[u8]> = body.chunks(7).collect();
    let run = run::<Signup>(&media_type, &chunks);
    assert_eq!(
        run.seen[0],
        Seen::Upload {
            path: "avatar".to_owned(),
            filename: "photo \"1\".png".to_owned(),
            media_type: Some("image/png".to_owned())
        }
    );
    assert_eq!(run.stored, [png.to_vec(), b"\r\n--".to_vec()]);
    let signup = run.accepted().content;
    assert_eq!(signup.name, "Zoë \"Z\" O'Neil\r\nthe second");
    assert_eq!(signup.plan, Some(Plan::Free));
    assert_eq!(
        signup.tags.as_deref(),
        Some(&["a".to_owned(), "b".to_owned()][..])
    );
    assert_eq!(
        signup.address,
        Some(Address {
            city: "Zürich".to_owned(),
            postcode: Some("8001".to_owned())
        })
    );
    assert_eq!(
        signup.lines,
        Some(vec![
            Line {
                sku: "X".to_owned(),
                quantity: None
            },
            Line {
                sku: "Y".to_owned(),
                quantity: Some(3)
            }
        ])
    );
    assert_eq!(
        signup.avatar.map(|avatar| *avatar.handle()),
        Some(1 as Handle)
    );
    assert_eq!(signup.documents.map(|documents| documents.len()), Some(1));
}
