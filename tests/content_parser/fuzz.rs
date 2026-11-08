// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Generated bodies, structured and arbitrary, checked for split invariance, no panics, the computed maximum, and the return of every handle.

use crate::support::{Handle, MEDIA_TYPE, Run, Seen, split_invariant_as};
use base_web_content::{Content, ContentFormat, ContentHandles, Upload};
use std::fmt::Debug;

const URL_ENCODED: &str = "application/x-www-form-urlencoded";

#[derive(Content, Debug)]
struct Tag {
    #[content(size_limit = 4)]
    label: Option<String>,
    #[content(size_limit = 4)]
    file: Option<Upload<Handle>>,
}

/// A declaration with every kind of field and small limits, which generated bodies reach.
#[derive(Content, Debug)]
#[content(
    preamble_size_limit = 6,
    epilogue_size_limit = 3,
    header_size_limit = 128,
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
    #[content(size_limit = 6)]
    b: Option<Vec<u8>>,
}

/// A declaration of two fields, whose computed maximum generated bodies often exceed.
#[derive(Content, Debug)]
#[content(
    preamble_size_limit = 2,
    epilogue_size_limit = 1,
    header_size_limit = 128,
    part_count_limit = 4,
    content_size_limit = 6,
    pair_count_limit = 3
)]
struct Tight {
    #[content(size_limit = 3)]
    a: Option<String>,
    #[content(size_limit = 4, accept("image/*"))]
    f: Option<Upload<Handle>>,
}

/// The handles a declaration's accepted content holds, checking that every value is within its limits.
trait Handles: Sized {
    fn handles(self) -> Vec<Handle>;
}

impl Handles for Fuzzed {
    fn handles(self) -> Vec<Handle> {
        assert!(self.a.is_none_or(|a| a.len() <= 5));
        // A `u8` is within its range by its type, which tests/field_value.rs covers.
        let _ = self.n;
        let list = self.l.unwrap_or_default();
        assert!(list.len() <= 2 && list.iter().all(|item| item.len() <= 4));
        assert!(self.b.unwrap_or_default().len() <= 6);
        let mut handles: Vec<Handle> = self.f.into_iter().map(Upload::into_handle).collect();
        let uploads = self.u.unwrap_or_default();
        assert!(uploads.len() <= 2 && uploads.iter().all(|upload| upload.size() <= 4));
        handles.extend(uploads.into_iter().map(Upload::into_handle));
        let tags = self.g.unwrap_or_default();
        assert!(tags.len() <= 2);
        for tag in tags {
            assert!(tag.label.is_none_or(|label| label.len() <= 4));
            handles.extend(tag.file.map(Upload::into_handle));
        }
        handles
    }
}

impl Handles for Tight {
    fn handles(self) -> Vec<Handle> {
        assert!(self.a.is_none_or(|a| a.len() <= 3));
        let file = self.f.inspect(|file| {
            assert!(file.size() <= 4 && !file.untrusted_filename().is_empty());
        });
        file.into_iter().map(Upload::into_handle).collect()
    }
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

    /// True with a probability of `percent` in 100.
    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    fn pick<'a>(&mut self, items: &[&'a [u8]]) -> &'a [u8] {
        items[self.below(items.len())]
    }
}

/// The names `Fuzzed` declares, which most parts use, and others it does not accept.
const FUZZED_NAMES: &[&[u8]] = &[
    b"a",
    b"n",
    b"f",
    b"l[]",
    b"u[]",
    b"g[][label]",
    b"g[][file]",
    b"b",
];
const TIGHT_NAMES: &[&[u8]] = &[b"a", b"f"];
const UNDECLARED: &[&[u8]] = &[
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

/// A name: usually one the declaration accepts.
fn name<'a>(random: &mut Random, names: &[&'a [u8]]) -> &'a [u8] {
    if random.chance(90) {
        random.pick(names)
    } else {
        random.pick(UNDECLARED)
    }
}

/// A multipart body of random parts: half of them well formed, with declared names and small content, so that they reach acceptance, uploads, and the computed maximum, and half with any framing, names, and damage.
fn multipart(random: &mut Random, names: &[&[u8]]) -> Vec<u8> {
    let hostile = random.chance(50);
    let mut body = Vec::new();
    if hostile && random.chance(20) {
        body.extend_from_slice(random.pick(&[b"pre\r\n", b"\r\n", b"preamble!\r\n", b"pre"]));
    }
    for _ in 0..random.below(7) {
        body.extend_from_slice(if !hostile || random.chance(85) {
            b"--XyZ\r\n"
        } else {
            random.pick(&[b"--XyZ \r\n", b"\r\n--XyZ\r\n", b"--XyZ\n"])
        });
        let name = if hostile {
            name(random, names)
        } else {
            random.pick(names)
        };
        let name = String::from_utf8_lossy(name).into_owned();
        let choice = random.below(if hostile { 40 } else { 38 });
        let disposition = match choice {
            0..=15 => format!("form-data; name=\"{name}\""),
            16..=19 => format!("form-data; name=\"{name}\"; filename=\"\""),
            20..=37 => format!("form-data; name=\"{name}\"; filename=\"x.png\""),
            38 => format!("form-data; name={name}; filename*=utf-8''x"),
            _ => format!("attachment; name=\"{name}\""),
        };
        body.extend_from_slice(format!("Content-Disposition: {disposition}\r\n").as_bytes());
        match random.below(6) {
            0..=2 => body.extend_from_slice(b"Content-Type: image/png\r\n"),
            3 => body.extend_from_slice(b"Content-Type: application/octet-stream\r\n"),
            4 if hostile => body.extend_from_slice(b"Content-Type: text/plain; charset=latin1\r\n"),
            _ => {}
        }
        body.extend_from_slice(b"\r\n");
        // A part with no file chosen has no content, unless the body is hostile.
        let pieces = match (hostile, (16..=19).contains(&choice)) {
            (false, true) => 0,
            (false, false) => 1,
            (true, _) => random.below(3),
        };
        for _ in 0..pieces {
            body.extend_from_slice(if !hostile {
                random.pick(&[b"1", b"ab", b"7"])
            } else {
                random.pick(CONTENT)
            });
        }
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(if !hostile || random.chance(70) {
        b"--XyZ--\r\n"
    } else {
        random.pick(&[
            b"--XyZ--",
            b"--XyZ--\r\nend",
            b"",
            b"--XyZ",
            b"--XyZ--  \r\n",
        ])
    });
    if hostile && random.chance(60) {
        damage(random, &mut body);
    }
    body
}

/// URL-encoded content of random pairs, with occasional damage.
fn url_encoded(random: &mut Random, names: &[&[u8]]) -> Vec<u8> {
    let mut body = Vec::new();
    for index in 0..random.below(7) {
        if index > 0 {
            body.push(b'&');
        }
        let name = name(random, names);
        body.extend(name.iter().flat_map(|&byte| match (byte, random.below(4)) {
            (b'[', 0) => b"%5B".to_vec(),
            (b']', 0) => b"%5D".to_vec(),
            (byte, _) => vec![byte],
        }));
        if random.chance(80) {
            body.push(b'=');
            for _ in 0..random.below(3) {
                body.extend_from_slice(random.pick(CONTENT));
            }
        }
    }
    if random.chance(40) {
        damage(random, &mut body);
    }
    body
}

/// Flip, insert, or remove a few bytes, or cut the body short.
fn damage(random: &mut Random, body: &mut Vec<u8>) {
    for _ in 0..1 + random.below(2) {
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

/// What a run of generated bodies reached, so that a test can check it exercised what it claims to.
#[derive(Default)]
struct Reached {
    accepted: usize,
    beyond_maximum: usize,
    stored: usize,
}

/// Check that the body gives the same result however it is split, that no body beyond the computed maximum is accepted, and that every handle the parser was given is returned.
fn check<T: ContentHandles<Handle> + Handles + Debug>(
    media_type: &str,
    format: ContentFormat,
    body: &[u8],
    reached: &mut Reached,
) {
    let run: Run<T> = split_invariant_as(media_type, body);
    let maximum = T::body_size_limit(format).unwrap();
    // A handle is given for each upload that ended; one whose content was still arriving has none.
    let stored = run
        .seen
        .iter()
        .filter(|seen| **seen == Seen::UploadEnd)
        .count() as Handle;
    let mut handles = match run.result {
        Ok(accepted) => {
            assert!(
                body.len() <= maximum,
                "accepted beyond the maximum: {body:?}"
            );
            reached.accepted += 1;
            let mut handles = accepted.content.handles();
            handles.extend(accepted.discarded);
            handles
        }
        Err(rejected) => rejected.handles,
    };
    handles.sort_unstable();
    assert_eq!(
        handles,
        (1..=stored).collect::<Vec<_>>(),
        "every handle is returned: {body:?}"
    );
    reached.beyond_maximum += usize::from(body.len() > maximum);
    reached.stored += stored as usize;
}

#[test]
fn generated_multipart_bodies() {
    let mut random = Random(seed(0x9e37_79b9_7f4a_7c15));
    let (mut fuzzed, mut tight) = (Reached::default(), Reached::default());
    let count = iterations(250);
    for _ in 0..count {
        let body = multipart(&mut random, FUZZED_NAMES);
        check::<Fuzzed>(
            MEDIA_TYPE,
            ContentFormat::MultipartFormData,
            &body,
            &mut fuzzed,
        );
        let body = multipart(&mut random, TIGHT_NAMES);
        check::<Tight>(
            MEDIA_TYPE,
            ContentFormat::MultipartFormData,
            &body,
            &mut tight,
        );
    }
    for reached in [&fuzzed, &tight] {
        assert!(
            reached.accepted > count / 20,
            "{} accepted",
            reached.accepted
        );
        assert!(
            reached.stored > count / 10,
            "{} uploads stored",
            reached.stored
        );
    }
    assert!(
        tight.beyond_maximum > count / 20,
        "{} beyond the maximum",
        tight.beyond_maximum
    );
}

#[test]
fn generated_url_encoded_content() {
    let mut random = Random(seed(0x2545_f491_4f6c_dd1d));
    let (mut fuzzed, mut tight) = (Reached::default(), Reached::default());
    let count = iterations(250);
    for _ in 0..count {
        let body = url_encoded(&mut random, FUZZED_NAMES);
        check::<Fuzzed>(URL_ENCODED, ContentFormat::UrlEncoded, &body, &mut fuzzed);
        let body = url_encoded(&mut random, TIGHT_NAMES);
        check::<Tight>(URL_ENCODED, ContentFormat::UrlEncoded, &body, &mut tight);
    }
    assert!(fuzzed.accepted > count / 20, "{} accepted", fuzzed.accepted);
    assert!(
        tight.beyond_maximum > count / 20,
        "{} beyond the maximum",
        tight.beyond_maximum
    );
}

#[test]
fn arbitrary_bytes() {
    let mut random = Random(seed(0xdead_beef_cafe_f00d));
    let mut reached = Reached::default();
    for _ in 0..iterations(150) {
        let alphabet = b"-\r\nXyZ%&=a[]\xff";
        let body: Vec<u8> = (0..random.below(48))
            .map(|_| alphabet[random.below(alphabet.len())])
            .collect();
        let raw: Vec<u8> = (0..random.below(48)).map(|_| random.next() as u8).collect();
        for body in [body, raw] {
            check::<Fuzzed>(
                MEDIA_TYPE,
                ContentFormat::MultipartFormData,
                &body,
                &mut reached,
            );
            check::<Fuzzed>(URL_ENCODED, ContentFormat::UrlEncoded, &body, &mut reached);
        }
    }
}
