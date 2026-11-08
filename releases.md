# Releases

## Unreleased

- Add `#[derive(Content)]`, from the new `base-web-content-macros` crate, to declare the fields, nested groups, arrays, and uploads a form accepts, with a size limit on every field, a count limit on every array, accepted media types for uploads, and limits on the whole body, checked when the declaration is compiled.
- Add `FieldValue`, the strict conversion of a field's text to `String`, integers, floats, `bool`, an enumeration with `#[derive(FieldValue)]`, or a type of your own.
- Add `Content::body_size_limit`, the largest body a declaration could need in each format, for a server's `Content-Length` check.
- Add `ContentParser`, which parses `multipart/form-data` bodies into a declared struct without I/O, framed by `base-web-multipart`: names the declaration does not accept are refused before their content is read, every limit, including the largest body the declaration could need, the header section, and the epilogue, is enforced as the bytes arrive, uploads are streamed to the caller as events and held by the caller's handles, which are all returned when parsing fails, and every invalid field is reported together, with its path and a code.
- Parse `application/x-www-form-urlencoded` content into the same declarations with `base-web-url`'s parser, refusing a name longer than any the declaration accepts before it is read whole, a `charset` other than UTF-8 or US-ASCII, and more pairs than the pair count limit; a pair without `=` is an `invalid_type` violation for any field.
- Treat a part with an empty `filename` for a declared upload, as browsers send for a file input with no file chosen, as no file: `accept` does not apply to it, it reports no upload and adds no array element, and content in it is refused with `ContentErrorKind::UnnamedFile`.
- Read the names of both formats as `base-web-url`'s bracket paths, and add `ContentParser::parse_end`, which takes the input that remains once it has ended, since URL-encoded content ends only where its input does.
