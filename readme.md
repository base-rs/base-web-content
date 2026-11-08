# `base-web-content`

Declarative, limited parsing of web request content, such as forms, without I/O, for Rust.

Applications declare the content they accept, as `multipart/form-data` or `application/x-www-form-urlencoded`, with `#[derive(Content)]`, with a limit on every field, array, and upload, so that a server never parses a body without knowing its limits. The parser is fed bytes as they arrive and refuses whatever the declaration does not accept as early as the bytes allow: an undeclared name before its content is read, and any limit as soon as it is exceeded. Uploads are streamed to storage the caller owns, and fields are converted strictly into the declared struct, with every invalid field reported together.

## Usage

```rust
use base_web_content::{Content, ContentEvent, ContentParser, Rejection};
use base_web_media_type::MediaType;

#[derive(Content)]
struct Login {
    #[content(size_limit = 256)]
    username: String,
    #[content(size_limit = 256)]
    password: String,
    remember: Option<bool>,
}

let media_type = MediaType::parse("multipart/form-data; boundary=XyZ")?;
let body = b"--XyZ\r\nContent-Disposition: form-data; name=\"username\"\r\n\r\nada\r\n--XyZ\r\nContent-Disposition: form-data; name=\"password\"\r\n\r\nhunter2\r\n--XyZ--\r\n";

let mut parser = ContentParser::<Login>::new(Some(&media_type))?;
// With the whole body in memory, its end is known: parse until the parser reports it.
let mut input = &body[..];
loop {
    // `finish` reports a fatal error again, with the handle of every upload stored so far, so parsing stops here rather than returning early.
    let Ok(parsed) = parser.parse_end(input) else { break };
    input = &input[parsed.consumed..];
    if parsed.event == ContentEvent::End {
        break;
    }
}
match parser.finish() {
    Ok(accepted) => assert_eq!((accepted.content.username.as_str(), accepted.content.remember), ("ada", None)),
    Err(rejected) => match rejected.reason {
        Rejection::Error(error) => panic!("respond with {}: {error}", error.status()),
        Rejection::Violations(violations) => panic!("show the form again with {} errors", violations.len()),
    },
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

See the [Getting Started](context/getting-started.md) guide for declarations, limits, uploads, and errors.

## Releases

See [releases.md](releases.md).

## See Also

- [Protocol::Content](https://github.com/socketry/protocol-content), the Ruby gem this crate follows.
- [`base-web-multipart`](https://github.com/base-rs/base-web-multipart), which frames the multipart bodies of forms.
- [`base-web-url`](https://github.com/base-rs/base-web-url), which reads URL-encoded forms and the bracket paths of field names.
- [`base-web-media-type`](https://github.com/base-rs/base-web-media-type), which reads the media types that choose a format and that uploads accept.
- [`base-http-content`](https://github.com/base-rs/base-http/tree/main/crates/content), which reads `base-http` requests with this crate and saves uploads to files safely.

## Contributing

Please open an issue or pull request on [GitHub](https://github.com/base-rs/base-web-content).

Install the [Bake launcher](https://github.com/base-rs/bake) and run `cargo bake test`. Use `cargo bake test:coverage --all-targets true` for the shared source-region coverage gate.

### Agent Context

Run `cargo bake agent:context:install` to install shared context and skills. Use `cargo bake agent:context:list` and `cargo bake agent:context:show` to discover relevant guides, follow repository guidance in `.agents/`, and apply skills under `.agents/skills/`.

Before planning or changing behavior, discover and read applicable `design/*.md`. Use `base-design-review` to assess plans and changes, and `base-design-author` when user requirements are introduced, changed, or removed. Review design compliance before opening or updating a pull request.
