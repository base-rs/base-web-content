// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Generated bodies, structured and arbitrary, checked for split invariance, no panics, the computed maximum, and the return of every handle.

use crate::support::{Handle, MEDIA_TYPE, split_invariant_as};
use base_web_content::{Content, ContentFormat, Upload};

#[derive(Content, Debug)]
struct Tag {
    #[content(size_limit = 4)]
    label: Option<String>,
    #[content(size_limit = 4)]
    file: Option<Upload<Handle>>,
}

#[derive(Content, Debug)]
#[content(
    preamble_size_limit = 6,
    epilogue_size_limit = 3,
    header_size_limit = 96,
    header_count_limit = 3,
    part_count_limit = 6,
    content_size_limit = 24,
    pair_count_limit = 6,
    depth_limit = 4
)]
struct Fuzzed {
    #[content(size_limit = 5)]
    a: Option<String>,
    #[content(size_limit = 3)]
    n: Option<u8>,
    #[content(size_limit = 5, accept("image/*"))]
    f: Option<Upload<Handle>>,
    #[content(count_limit = 2, size_limit = 4)]
    l: Option<Vec<String>>,
    #[content(count_limit = 2, size_limit = 4)]
    u: Option<Vec<Upload<Handle>>>,
    #[content(count_limit = 2)]
    g: Option<Vec<Tag>>,
    b: Option<Vec<u8>>,
}

/// The bodies to generate: `count`, or as many as `FUZZ_ITERATIONS` asks for a longer run.
fn iterations(count: usize) -> usize {
    std::env::var("FUZZ_ITERATIONS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(count)
}

/// The seed: fixed, or `FUZZ_SEED` for a different run.
fn seed(fixed: u64) -> u64 {
    std::env::var("FUZZ_SEED")
        .ok()
        .and_then(|value| value.parse().ok())
        .map_or(fixed, |seed: u64| seed | 1)
}

struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }

    fn pick<'a>(&mut self, items: &[&'a [u8]]) -> &'a [u8] {
        items[self.below(items.len())]
    }
}

const NAMES: &[&[u8]] = &[
    b"a",
    b"n",
    b"f",
    b"l[]",
    b"u[]",
    b"g[][label]",
    b"g[][file]",
    b"b",
    b"x",
    b"a[]",
    b"g[0][label]",
    b"l",
    b"g[]",
    b"a[b][c][d][e]",
    b"",
    b"[a]",
    b"a]",
];
const CONTENT: &[&[u8]] = &[
    b"",
    b"1",
    b"ab",
    b"12345",
    b"123456",
    b"\r",
    b"\n",
    b"\r\n",
    b"-",
    b"--XyZ",
    b"\r\n--Xy",
    b"\n--XyZ",
    b"\xff",
    b"%",
    b"%4",
    b"%41",
    b"+",
    b"&",
    b"=",
];

/// A multipart body of random parts, framing, and damage.
fn multipart(random: &mut Random) -> Vec<u8> {
    let mut body = Vec::new();
    if random.below(4) == 0 {
        body.extend_from_slice(random.pick(&[b"pre", b"\r\n", b"preamble!", b"\n"]));
    }
    for _ in 0..random.below(5) {
        body.extend_from_slice(random.pick(&[
            b"--XyZ\r\n",
            b"--XyZ \r\n",
            b"\r\n--XyZ\r\n",
            b"--XyZ\n",
        ]));
        let name = String::from_utf8_lossy(random.pick(NAMES)).into_owned();
        let disposition = match random.below(5) {
            0 => format!("form-data; name=\"{name}\""),
            1 => format!("form-data; name=\"{name}\"; filename=\"\""),
            2 => format!("form-data; name=\"{name}\"; filename=\"x.png\""),
            3 => format!("form-data; name={name}; filename*=utf-8''x"),
            _ => format!("attachment; name=\"{name}\""),
        };
        body.extend_from_slice(format!("Content-Disposition: {disposition}\r\n").as_bytes());
        match random.below(4) {
            0 => body.extend_from_slice(b"Content-Type: image/png\r\n"),
            1 => body.extend_from_slice(b"Content-Type: application/octet-stream\r\n"),
            2 => body.extend_from_slice(b"Content-Type: text/plain; charset=latin1\r\n"),
            _ => {}
        }
        body.extend_from_slice(b"\r\n");
        for _ in 0..random.below(3) {
            body.extend_from_slice(random.pick(CONTENT));
        }
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(random.pick(&[
        b"--XyZ--\r\n",
        b"--XyZ--",
        b"--XyZ--\r\nend",
        b"",
        b"--XyZ",
        b"--XyZ--  \r\n",
    ]));
    damage(random, &mut body);
    body
}

/// URL-encoded content of random pairs and damage.
fn url_encoded(random: &mut Random) -> Vec<u8> {
    let mut body = Vec::new();
    for index in 0..random.below(7) {
        if index > 0 {
            body.push(b'&');
        }
        let name = random.pick(NAMES);
        body.extend(name.iter().flat_map(|&byte| match (byte, random.below(4)) {
            (b'[', 0) => b"%5B".to_vec(),
            (b']', 0) => b"%5D".to_vec(),
            (byte, _) => vec![byte],
        }));
        if random.below(5) != 0 {
            body.push(b'=');
            for _ in 0..random.below(3) {
                body.extend_from_slice(random.pick(CONTENT));
            }
        }
    }
    damage(random, &mut body);
    body
}

/// Flip, insert, or remove a few bytes, or cut the body short.
fn damage(random: &mut Random, body: &mut Vec<u8>) {
    for _ in 0..random.below(3) {
        if body.is_empty() {
            break;
        }
        let index = random.below(body.len());
        match random.below(4) {
            0 => body[index] = random.next() as u8,
            1 => body.insert(
                index,
                random.pick(&[b"\r", b"\n", b"-", b"%", b"&", b"="])[0],
            ),
            2 => {
                body.remove(index);
            }
            _ => body.truncate(index),
        }
    }
}

/// The handles a parse returned, sorted: those in the result and those discarded, or those of a refusal.
fn returned(run: crate::support::Run<Fuzzed>) -> Vec<Handle> {
    let mut handles = match run.result {
        Ok(accepted) => {
            let content = accepted.content;
            // Every accepted value is within its limits.
            assert!(content.a.as_ref().is_none_or(|a| a.len() <= 5));
            // A `u8` is within its range by its type, which tests/field_value.rs covers.
            let _ = content.n;
            let list = content.l.unwrap_or_default();
            assert!(list.len() <= 2 && list.iter().all(|item| item.len() <= 4));
            assert!(
                content.b.unwrap_or_default().len() <= 24,
                "within the total content limit"
            );
            let mut handles = accepted.discarded;
            handles.extend(content.f.map(Upload::into_handle));
            handles.extend(content.u.into_iter().flatten().map(Upload::into_handle));
            for tag in content.g.into_iter().flatten() {
                assert!(tag.label.is_none_or(|label| label.len() <= 4));
                handles.extend(tag.file.map(Upload::into_handle));
            }
            handles
        }
        Err(rejected) => rejected.handles,
    };
    handles.sort_unstable();
    handles
}

fn check(media_type: &str, format: ContentFormat, body: &[u8]) {
    let run = split_invariant_as::<Fuzzed>(media_type, body);
    let accepted = run.result.is_ok();
    // A handle is given for each upload that ended; one whose content was still arriving has none.
    let stored = run
        .seen
        .iter()
        .filter(|seen| **seen == crate::support::Seen::UploadEnd)
        .count() as Handle;
    let maximum = Fuzzed::body_size_limit(format).unwrap();
    assert!(
        !accepted || body.len() <= maximum,
        "accepted beyond the maximum: {body:?}"
    );
    assert_eq!(
        returned(run),
        (1..=stored).collect::<Vec<_>>(),
        "every handle is returned: {body:?}"
    );
}

#[test]
fn generated_multipart_bodies() {
    let mut random = Random(seed(0x9e37_79b9_7f4a_7c15));
    for _ in 0..iterations(600) {
        check(
            MEDIA_TYPE,
            ContentFormat::MultipartFormData,
            &multipart(&mut random),
        );
    }
}

#[test]
fn generated_url_encoded_content() {
    let mut random = Random(seed(0x2545_f491_4f6c_dd1d));
    for _ in 0..iterations(600) {
        check(
            "application/x-www-form-urlencoded",
            ContentFormat::UrlEncoded,
            &url_encoded(&mut random),
        );
    }
}

#[test]
fn arbitrary_bytes() {
    let mut random = Random(seed(0xdead_beef_cafe_f00d));
    for _ in 0..iterations(300) {
        let alphabet = b"-\r\nXyZ%&=a[]\xff";
        let body: Vec<u8> = (0..random.below(48))
            .map(|_| alphabet[random.below(alphabet.len())])
            .collect();
        check(MEDIA_TYPE, ContentFormat::MultipartFormData, &body);
        check(
            "application/x-www-form-urlencoded",
            ContentFormat::UrlEncoded,
            &body,
        );
        let raw: Vec<u8> = (0..random.below(48)).map(|_| random.next() as u8).collect();
        check(MEDIA_TYPE, ContentFormat::MultipartFormData, &raw);
        check(
            "application/x-www-form-urlencoded",
            ContentFormat::UrlEncoded,
            &raw,
        );
    }
}
