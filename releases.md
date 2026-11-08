# Releases

## Unreleased

- Add `#[derive(Content)]`, from the new `base-web-content-macros` crate, to declare the fields, nested groups, arrays, and uploads a form accepts, with a size limit on every field, a count limit on every array, accepted media types for uploads, and limits on the whole body, checked when the declaration is compiled.
- Add `FieldValue`, the strict conversion of a field's text to `String`, integers, floats, `bool`, an enumeration with `#[derive(FieldValue)]`, or a type of your own.
- Add `Content::body_size_limit`, the largest body a declaration could need in each format, for a server's `Content-Length` check.
