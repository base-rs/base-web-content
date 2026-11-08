// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

// Only the declarations' limits are tested here, so their fields are never read.
#![allow(dead_code)]

use base_web_content::{Content, ContentFormat, Upload};
use base_web_media_type::MediaType;

const MULTIPART: ContentFormat = ContentFormat::MultipartFormData;
const URL_ENCODED: ContentFormat = ContentFormat::UrlEncoded;

#[derive(Content)]
struct Login {
    username: String,
    password: String,
}

#[test]
fn defaults() {
    // The preamble, each part's boundary line, header section, and empty line, the content, the closing boundary line, and the epilogue.
    let multipart = 65536 + 2 * (76 + 8192 + 2) + 2 * 65536 + 78 + 1024;
    assert_eq!(Login::body_size_limit(MULTIPART), Some(multipart));
    // Each pair's name and content, percent-encoded, with `=` and `&`.
    let pair = 3 * "username".len() + 3 * 65536 + 2;
    assert_eq!(Login::body_size_limit(URL_ENCODED), Some(2 * pair));
}

#[derive(Content)]
struct Line {
    #[content(size_limit = 5)]
    sku: String,
    #[content(count_limit = 2, size_limit = 1)]
    flags: Vec<String>,
}

#[derive(Content)]
#[content(
    header_size_limit = 1000,
    preamble_size_limit = 0,
    epilogue_size_limit = 0
)]
struct Order {
    #[content(size_limit = 10)]
    note: Option<String>,
    #[content(count_limit = 3)]
    lines: Vec<Line>,
    #[content(size_limit = 100)]
    receipt: Option<Upload<()>>,
}

/// `Order`, limited to fewer parts and pairs than it declares.
#[derive(Content)]
#[content(
    header_size_limit = 1000,
    preamble_size_limit = 0,
    epilogue_size_limit = 0,
    part_count_limit = 2,
    pair_count_limit = 2
)]
struct ShortOrder {
    #[content(size_limit = 10)]
    note: Option<String>,
    #[content(count_limit = 3)]
    lines: Vec<Line>,
    #[content(size_limit = 100)]
    receipt: Option<Upload<()>>,
}

#[test]
fn counts_nested_groups_and_arrays() {
    // 11 parts: the note, 3 lines of a SKU and 2 flags each, and the receipt; 131 bytes of content: 10, 3 × (5 + 2 × 1), and 100.
    assert_eq!(
        Order::body_size_limit(MULTIPART),
        Some(11 * (76 + 1000 + 2) + 131 + 78)
    );
    // `note`: 3 × 4 + 3 × 10 + 2; `lines[][sku]`: 3 × 12 + 3 × 5 + 2, three times; `lines[][flags][]`: 3 × 16 + 3 × 1 + 2, six times; `receipt`: 3 × 7 + 3 × 100 + 2.
    assert_eq!(
        Order::body_size_limit(URL_ENCODED),
        Some(44 + 3 * 53 + 6 * 53 + 323)
    );
}

#[test]
fn counts_only_the_parts_and_pairs_a_body_may_have() {
    assert_eq!(
        ShortOrder::body_size_limit(MULTIPART),
        Some(2 * (76 + 1000 + 2) + 131 + 78)
    );
    // The two largest pairs: the receipt and one of the lines.
    assert_eq!(ShortOrder::body_size_limit(URL_ENCODED), Some(323 + 53));
}

#[derive(Content)]
#[content(content_size_limit = 100, url_encoded_size_limit = 1000)]
struct Capped {
    #[content(count_limit = 1000)]
    values: Vec<String>,
}

#[test]
fn caps_content_and_url_encoded_bodies() {
    // 128 parts, the default part count limit, and 100 bytes of content, the total content limit.
    assert_eq!(
        Capped::body_size_limit(MULTIPART),
        Some(65536 + 128 * 8270 + 100 + 78 + 1024)
    );
    assert_eq!(Capped::body_size_limit(URL_ENCODED), Some(1000));
}

#[derive(Content)]
#[content(content_size_limit = usize::MAX, part_count_limit = usize::MAX, pair_count_limit = usize::MAX, url_encoded_size_limit = usize::MAX)]
struct Unbounded {
    #[content(count_limit = usize::MAX)]
    groups: Vec<Huge>,
}

#[derive(Content)]
struct Huge {
    #[content(size_limit = usize::MAX, count_limit = usize::MAX)]
    values: Vec<String>,
}

#[test]
fn saturates_instead_of_overflowing() {
    assert_eq!(Unbounded::body_size_limit(MULTIPART), Some(usize::MAX));
    assert_eq!(Unbounded::body_size_limit(URL_ENCODED), Some(usize::MAX));
}

#[derive(Content)]
#[content(formats("multipart/form-data"))]
struct MultipartOnly {
    file: Upload<()>,
}

#[derive(Content)]
#[content(formats("application/x-www-form-urlencoded"))]
struct UrlEncodedOnly {
    query: String,
}

#[test]
fn only_for_accepted_formats() {
    assert!(MultipartOnly::body_size_limit(MULTIPART).is_some());
    assert_eq!(MultipartOnly::body_size_limit(URL_ENCODED), None);
    assert_eq!(UrlEncodedOnly::body_size_limit(MULTIPART), None);
    assert!(UrlEncodedOnly::body_size_limit(URL_ENCODED).is_some());
}

#[test]
fn formats_from_media_types() {
    let format = |text| ContentFormat::from_media_type(&MediaType::parse(text).unwrap());
    assert_eq!(format("multipart/form-data; boundary=x"), Some(MULTIPART));
    assert_eq!(format("Multipart/Form-Data"), Some(MULTIPART));
    assert_eq!(
        format("application/x-www-form-urlencoded; charset=utf-8"),
        Some(URL_ENCODED)
    );
    assert_eq!(format("multipart/mixed; boundary=x"), None);
    assert_eq!(format("application/json"), None);
    assert_eq!(MULTIPART.media_type(), MediaType::MULTIPART_FORM_DATA);
    assert_eq!(
        URL_ENCODED.media_type(),
        MediaType::APPLICATION_X_WWW_FORM_URLENCODED
    );
}
