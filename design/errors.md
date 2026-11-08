# Errors

## Requirement

Parsing must report two kinds of failure.

A fatal error stops parsing. It must give the offset in the body at which it was found and a kind that maps to exactly one HTTP status:

- 415 Unsupported Media Type: a missing or unsupported media type, including a URL-encoded body with an unsupported `charset`, or a format the declaration does not accept;
- 413 Content Too Large: any limit in `design/limits.md`, including an upload over its size limit and a name deeper than the depth limit, which is reported as such even though the declaration cannot accept it either;
- 400 Bad Request: anything else, including malformed multipart framing, a missing or invalid boundary, a part without a `form-data` name, a malformed URL-encoded pair, and a name the declaration does not accept.

Once the parser reports a fatal error, it must report the same error again. Errors from `base-web-multipart` and `base-web-url` must be reported as fatal errors of this crate with the same offset.

A violation is a field that arrived but cannot be accepted, or a required field that did not arrive. It must give the field's path, the declared names and array indices that lead to it, and a code: `required`, `invalid_type` for a value that does not convert, is not valid UTF-8, has an unsupported charset, or is a file where text is declared or the reverse, and `unsupported_media_type` for an upload `accept` refuses. Violations do not stop parsing: every violation in the body must be collected and returned together when it ends, so that an application can present them all. A required upload that arrived but was refused must report only its refusal, not also `required`.

Errors, violations, and the `Debug` output of every type in the crate must never include submitted content, field values, filenames, media types, or names that are not declared: only offsets, kinds, codes, declared names, indices, limits, and what the declaration accepts.

## Rationale

These decisions were made with the user on 2026-11-08, drawing on the Ruby gem `protocol-content` 0.2.0. The user decided on fatal errors that are sticky, give a byte offset, never contain content, and map cleanly to 400, 413, and 415, as `base-web-multipart`'s errors do, and on violations with a path and code. The gem's violations keep the submitted value in their details (`ValueField#apply` in `lib/protocol/content/parameters/field.rb`), which the user decided this crate must not do, so that logging an error cannot leak a password or personal data. The rule extends to names that are not declared, since they are submitted too.

The user asked that whether to collect violations follow the gem, which collects them so that all failures can be shown together (`Model#apply` in `lib/protocol/content/parameters/model.rb`, and its guide, `guides/parameters/readme.md`), while the failures the user decided are strict, undeclared names and limits, stay fatal. The codes are the gem's (`required`, `invalid_type`, `unsupported_media_type`); its `unknown` and `too_large` are fatal here. Reporting only the refusal of a required upload follows `UploadField#apply_multiple`. The gem maps limit errors to `ContentTooLargeError` (`lib/protocol/content/default.rb`), which 413 follows, including for depth.

## Scope

The crate's fatal error and violation types, their codes and status mapping, and the `Debug` output of every public type.

## Validation

Tests that each fatal case gives its kind, status, and offset, and the same error when parsing continues; that a body with several violations reports all of them, in order, with their paths; and that formatting every error, violation, upload, and event with `Display` and `Debug` contains none of a test body's distinctive values, filenames, media types, or undeclared names.
