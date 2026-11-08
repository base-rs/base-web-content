# Scope

## Requirement

`base-web-content` must parse the content of web requests into structures an application declares in advance, with a limit on everything it accepts, so that a server never parses a body without knowing its limits. Parsing must be strict and proactive: whatever the declaration does not accept is refused as early as the bytes allow, rather than read and checked afterwards.

The crate must do no I/O. It must be fed bytes, as `base-web-multipart`'s `MultipartParser` is, and must not depend on an async runtime or perform file, network, or clock operations. Reading a request body, the early `Content-Length` check, and saving uploads to files belong to the server layer, `base-http-server` or wherever request handling lives, not here.

The crate must parse exactly these formats, chosen by the request's media type:

- `multipart/form-data`, framed by `base-web-multipart`;
- `application/x-www-form-urlencoded`, parsed by `base-web-url`, which shares that parser with query strings.

The list is fixed and internal: applications cannot register formats. Any other media type, or none, must be refused as unsupported. JSON is not supported.

Declarations must be written with `#[derive(Content)]`, provided by the companion proc-macro crate `base-web-content-macros` in the same workspace and released with it at the same version, as `base-web-html` and `base-web-html-macros` are. The runtime model the derive generates must not be public API: applications declare content only through the derive.

## Rationale

These decisions were made with the user on 2026-11-08, drawing on the Ruby gem `protocol-content` 0.2.0. The gem parses content first and then applies a parameter model to the result (`lib/protocol/content/parameters/model.rb`, `Model#parse`), so a body is read before anything checks whether its fields were wanted. The user wanted applications to declare up front exactly what they accept, with limits, and the parser to refuse the rest while it reads; this crate is deliberately stricter than the gem in that respect.

The gem dispatches on media type through a registry that applications extend (`lib/protocol/content/parser.rb`, `Parser#register`), with JSON, URL-encoded, and multipart parsers registered by default (`lib/protocol/content/default.rb`). The user chose a fixed list for 0.1: opening it to extension is future work, likely after JSON, and JSON will come later as an incremental, streaming parser of this project's own with a way to represent file uploads, rather than buffering a document as the gem's `lib/protocol/content/json_parser.rb` does. The URL-encoded parser belongs in `base-web-url`, as the gem uses `protocol-url`'s `lib/protocol/url/form_data/parser.rb`; writing it is a separate step from this crate.

`base-web-multipart`'s `design/scope.md` leaves form fields, uploads, and content limits to this crate. The user decided that the derive's runtime model stays internal for now; it may become public later if applications need forms whose fields are only known at run time, which the gem's builder (`lib/protocol/content/parameters/builder.rb`) allows. Saving uploads safely and adapting async request bodies were placed in the server layer so that this crate stays sans-I/O.

## Scope

The `base-web-content` and `base-web-content-macros` crates as a whole.

## Validation

Inspect `cargo tree -e normal` for both crates: no async runtime or I/O crate, and `base-web-multipart` and `base-web-url` present once the formats are implemented. `getrandom`, which `base-web-multipart` uses to generate boundaries for its writer, is not called by this crate. Review public items against this scope: a public builder for the runtime model, a format registry, JSON, file or socket access, or a `Content-Length` check belong elsewhere or need a design change first. Exercise parsing with an unsupported media type and with none, and check that both are refused.
