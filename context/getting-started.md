# Getting Started

This guide explains how to declare the content a request may carry and parse it with `base-web-content`.

## Declaring content

An application declares the content it accepts by deriving `Content` on a struct. Each field is one form field, named by its identifier unless `rename` gives the name a form submits, and every field has a limit:

```rust
use base_web_content::{Content, FieldValue, Upload};

#[derive(FieldValue, Debug, PartialEq)]
enum Plan {
    #[content(rename = "free")]
    Free,
    #[content(rename = "pro")]
    Pro,
}

#[derive(Content)]
struct Address {
    city: String,
    postcode: Option<String>,
}

#[derive(Content)]
struct Signup {
    // Text, which must be valid UTF-8, up to 64 KiB unless a size limit says otherwise.
    #[content(size_limit = 256)]
    email: String,
    // Converted strictly: `36` or `+36`, but not ` 36` or `36.0`.
    age: Option<u8>,
    // One of the spellings the enumeration lists.
    plan: Plan,
    // A nested group, submitted as `address[city]` and `address[postcode]`.
    address: Option<Address>,
    // An array, submitted as `interests[]`, of up to 8 elements.
    #[content(count_limit = 8)]
    interests: Vec<String>,
    // A file, accepted only with one of these media types, up to 4 MiB.
    #[content(accept("image/png", "image/jpeg"), size_limit = 4 * 1024 * 1024)]
    avatar: Option<Upload<std::path::PathBuf>>,
}
```

A field without `Option` is required. `Vec<u8>` holds the submitted bytes unchanged rather than being an array, and an array of nested groups is submitted as `name[][field]`, where an element ends when a field it already has arrives again. Only the field named after `[]` decides this, so an array within an array of groups, such as `items[][tags][]`, holds one element in each group: a second tag begins the next item. Implement `FieldValue` for a type of your own to accept it as a field.

The struct may override the limits on the whole body, such as `#[content(part_count_limit = 16, epilogue_size_limit = 0)]`, and restrict the formats it accepts with `#[content(formats("multipart/form-data"))]`. The derive refuses, when the declaration compiles, a field type it cannot parse, two fields with the same name, names deeper than the depth limit, and attributes that do not suit their field.

## Refusing a body early

From its fields and limits, a declaration knows the largest body it could need in each format. A server can refuse a request whose `Content-Length` exceeds it before reading the body:

```rust
use base_web_content::{Content, ContentFormat};

#[derive(Content)]
struct Search {
    #[content(size_limit = 100)]
    query: String,
}

let content_length = 1_000_000;
let maximum = Search::body_size_limit(ContentFormat::MultipartFormData).expect("the format is accepted");
assert!(content_length > maximum, "respond with 413 Content Too Large");
```

The parser enforces the same maximum as a running total, whatever the request declared, so a body that repeats its fields cannot exceed it either.

## Parsing a body as it arrives

`ContentParser` does no I/O. Create one for the request's media type, which chooses the format and refuses an unsupported or missing one, then feed it the bytes you have. It reports how many it consumed and what they held: drop the consumed bytes, append what arrives next, and call again. When the input ends, `finish` returns the declared struct, or why it was refused:

```rust
use base_web_content::{Content, ContentEvent, ContentParser};
use base_web_media_type::MediaType;

#[derive(Content)]
struct Comment {
    author: String,
    #[content(size_limit = 1024)]
    body: String,
}

// The body as it might arrive from a socket, in arbitrary pieces.
let chunks: [&[u8]; 3] = [
    b"--XyZ\r\nContent-Disposition: form-data; name=\"author\"\r\n\r\nAda\r\n--XyZ\r\nContent-Disp",
    b"osition: form-data; name=\"body\"\r\n\r\nFirst!",
    b"\r\n--XyZ--\r\n",
];

let media_type = MediaType::parse("multipart/form-data; boundary=XyZ")?;
let mut parser = ContentParser::<Comment>::new(Some(&media_type))?;
let mut buffer = Vec::new();
for chunk in chunks {
    buffer.extend_from_slice(chunk);
    loop {
        let parsed = parser.parse(&buffer)?;
        // Wait for more input when the parser needs it, or when the body has ended and the input is consumed.
        let waiting = parsed.event == ContentEvent::Incomplete
            || (parsed.event == ContentEvent::End && parsed.consumed == buffer.len());
        let consumed = parsed.consumed;
        buffer.drain(..consumed);
        if waiting {
            break;
        }
    }
}
// The input has ended: give the parser what remains until it reports the end.
loop {
    let parsed = parser.parse_end(&buffer)?;
    let ended = parsed.event == ContentEvent::End;
    let consumed = parsed.consumed;
    buffer.drain(..consumed);
    if ended {
        break;
    }
}
let comment = parser.finish().expect("the comment is accepted").content;
assert_eq!((comment.author.as_str(), comment.body.as_str()), ("Ada", "First!"));
# Ok::<(), Box<dyn std::error::Error>>(())
```

When the input ends, `parse_end` takes what remains, and never asks for more: it reports the end, or `UnexpectedEnd` for content that is not complete, such as a multipart body without its closing boundary. URL-encoded content ends only where its input does, so its last pair is only complete when the end is given.

Fields other than uploads are buffered by the parser within their limits and converted when parsing finishes, so the only events are an upload's and the body's end. A part's header section is read whole, so your buffer must be able to hold `ContentParser::minimum_buffer_size()` bytes.

The result, events, violations, and errors are the same however the body is split, except that an upload's content may arrive in different pieces.

## Uploads

An upload's content is not buffered: the parser reports it as it arrives, borrowed from your buffer, for you to store wherever you choose. When the upload ends, give the parser a handle for what you stored, which the result holds in its `Upload`:

```rust
use base_web_content::{Content, ContentEvent, ContentParser, Upload};
use base_web_media_type::MediaType;

#[derive(Content)]
struct Avatar {
    #[content(accept("image/*"), size_limit = 1024)]
    avatar: Upload<usize>,
}

let media_type = MediaType::parse("multipart/form-data; boundary=XyZ")?;
let body = b"--XyZ\r\nContent-Disposition: form-data; name=\"avatar\"; filename=\"me.png\"\r\nContent-Type: image/png\r\n\r\n\x89PNG\r\n--XyZ--\r\n";

// Stand-ins for files, by handle.
let mut files: Vec<Vec<u8>> = Vec::new();
let mut parser = ContentParser::<Avatar, usize>::new(Some(&media_type))?;
let mut input = &body[..];
loop {
    let parsed = parser.parse_end(input)?;
    input = &input[parsed.consumed..];
    match parsed.event {
        ContentEvent::Upload(head) => {
            // The filename and media type are what the client declared, and must not be trusted.
            assert_eq!(head.untrusted_filename(), "me.png");
            files.push(Vec::new());
        }
        ContentEvent::UploadContent(content) => files.last_mut().unwrap().extend_from_slice(content),
        ContentEvent::UploadEnd => parser.store(files.len() - 1),
        ContentEvent::End => break,
        ContentEvent::Incomplete => unreachable!("parse_end is given the rest of the input"),
    }
}
let avatar = parser.finish().expect("the avatar is accepted").content.avatar;
assert_eq!((avatar.size(), &files[*avatar.handle()][..]), (4, &b"\x89PNG"[..]));
# Ok::<(), Box<dyn std::error::Error>>(())
```

An upload is reported only when its name is declared, its part has a `filename`, and its declared `Content-Type` is one `accept` allows. The media type is never guessed from the filename. Any other file, and its content, is read and discarded within its field's size limit, and recorded as a violation.

A browser submits a file input with no file chosen as a part with an empty `filename` and no content. Such a part is no file at all: `accept` does not apply to it, no upload is reported, an `Option<Upload<H>>` is `None`, a `Vec<Upload<H>>` gains no element, and a required upload is reported as `required`. A part with an empty `filename` that has content is refused with `ContentErrorKind::UnnamedFile`.

Uploads are stored before the whole body has been validated. When parsing fails, `Rejected::handles` holds every handle you gave the parser, so that you can remove what you stored, and an upload whose content was still arriving has no handle, so remove what you stored of it too; when a later upload for the same field replaces one, the earlier handle is returned in `Accepted::discarded`. Saving uploads safely, such as with an exclusive create and restrictive permissions, belongs to your server.

## Errors and violations

Parsing fails in one of two ways, each with a status for the response:

- A fatal `ContentError` stops parsing at the offset in the body where it was found, and the parser reports it again for every later call. Its `status()` is 415 for a missing, unsupported, or unaccepted media type, 413 for any limit, and 400 for anything else, such as malformed framing or a name the declaration does not accept.
- `Violation`s are fields that arrived but cannot be accepted, or required fields that did not. They do not stop parsing: every one is collected, in declaration order, with its path and a code, `required`, `invalid_type`, or `unsupported_media_type`.

```rust
use base_web_content::{Content, ContentEvent, ContentParser, Rejection};
use base_web_media_type::MediaType;

#[derive(Content)]
struct Order {
    quantity: u32,
    #[content(count_limit = 4)]
    items: Vec<u64>,
}

let media_type = MediaType::parse("multipart/form-data; boundary=XyZ")?;
let body = b"--XyZ\r\nContent-Disposition: form-data; name=\"items[]\"\r\n\r\n12\r\n--XyZ\r\nContent-Disposition: form-data; name=\"items[]\"\r\n\r\ntwelve\r\n--XyZ--\r\n";
let mut parser = ContentParser::<Order>::new(Some(&media_type))?;
let mut input = &body[..];
loop {
    let parsed = parser.parse_end(input)?;
    input = &input[parsed.consumed..];
    if parsed.event == ContentEvent::End {
        break;
    }
}
let Rejection::Violations(violations) = parser.finish().expect_err("the order is refused").reason else {
    unreachable!("the body parsed");
};
let violations: Vec<String> = violations.iter().map(ToString::to_string).collect();
assert_eq!(violations, ["quantity: required", "items[1]: invalid_type"]);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Errors, violations, and the `Debug` output of every type in the crate never include submitted content, values, filenames, media types, or names the declaration does not accept, so they are safe to log.

## Formats

`multipart/form-data` is parsed with [`base-web-multipart`](https://github.com/base-rs/base-web-multipart), reading part names and filenames under the `ParameterRules` of form data, which refuse `filename*`; `ContentParser::with_parameter_rules` overrides them.

`application/x-www-form-urlencoded` is parsed with [`base-web-url`](https://github.com/base-rs/base-web-url), into the same declarations. A name is refused before its value is read, and as soon as it is longer than three times the longest name the declaration accepts, the most percent-encoding could make it. A pair without `=`, such as `remember` in `remember&name=Ada`, has no value, which no field type accepts, while `remember=` has an empty one. Values are UTF-8, or US-ASCII when the media type's `charset` says so; any other `charset` is refused as unsupported:

```rust
use base_web_content::{Content, ContentEvent, ContentParser};
use base_web_media_type::MediaType;

#[derive(Content)]
struct Search {
    #[content(size_limit = 100)]
    q: String,
    #[content(count_limit = 8)]
    tags: Option<Vec<String>>,
    page: Option<u32>,
}

let media_type = MediaType::parse("application/x-www-form-urlencoded")?;
let mut parser = ContentParser::<Search>::new(Some(&media_type))?;
let mut input = &b"q=caf%C3%A9+au+lait&tags[]=drinks&tags%5B%5D=french&page=2"[..];
loop {
    let parsed = parser.parse_end(input)?;
    input = &input[parsed.consumed..];
    if parsed.event == ContentEvent::End {
        break;
    }
}
let search = parser.finish().expect("the search is accepted").content;
assert_eq!(search.q, "café au lait");
assert_eq!(search.tags.as_deref(), Some(&["drinks".to_owned(), "french".to_owned()][..]));
assert_eq!(search.page, Some(2));
# Ok::<(), Box<dyn std::error::Error>>(())
```

The names of both formats are read as `base-web-url` reads bracket paths, after decoding, so `tags%5B%5D` is `tags[]`.
