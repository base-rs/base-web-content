# Limits

## Requirement

Everything a declaration accepts must have a limit, with a default that the declaration may override:

| Limit | Applies to | Default |
| --- | --- | --- |
| `size_limit` | The content of one text, bytes, or converted scalar field | 64 KiB |
| `size_limit` | The content of one upload | 128 MiB |
| `count_limit` | The elements of one array | 128 |
| Total content | The content of every field and upload together | 256 MiB |
| Depth | The components of one name | 8 |
| Part count | The parts of a `multipart/form-data` body | 128 |
| Header section | One part's header section, including padding after its boundary | 8 KiB |
| Header count | The fields in one part's header section | 64 |
| Preamble | The bytes before the first boundary | 64 KiB |
| Epilogue | The bytes after the closing boundary line | 1 KiB |
| Pair count | The pairs of an `application/x-www-form-urlencoded` body | 1,024 |
| URL-encoded body | An `application/x-www-form-urlencoded` body | 2 MiB |

The derive must compute, from the declaration and its limits, the largest body each accepted format could need, and the crate must expose it, so that the server layer can refuse a request whose `Content-Length` exceeds it before reading the body. The number of parts or pairs a declaration could accept is the smaller of the format's count limit and the sum of its fields' counts, one for each scalar or upload and `count_limit` for each array, within nested groups as well. For `multipart/form-data`, the maximum is:

- the preamble limit;
- for each part it could accept, a boundary line of at most 76 bytes (a CRLF, `--`, a boundary of at most 70 characters as RFC 2046 allows, and a CRLF) and the header section limit;
- the smaller of the sum of the declared content limits and the total content limit;
- the closing boundary line and the epilogue limit.

For `application/x-www-form-urlencoded`, it is the smaller of the URL-encoded body limit and the sum, over each pair it could accept, of three times the bytes of the longest name the pair could have, three times its content limit, since percent-encoding at most triples a byte, a `=`, and a `&`. The computation must not overflow: a sum that would overflow saturates at `usize::MAX`.

The parser must enforce each limit while parsing, as soon as the bytes show it is exceeded, and must not depend on the early check: the maximum is coarse, and enforcement during parsing is authoritative. The total content, part count, and pair count limits, and the computed maximum body size, must be enforced as running totals across the whole body, so that a body the declaration could not accept is refused however its parts are arranged. Exceeding any limit is fatal, as `design/errors.md` describes.

## Rationale

These decisions were made with the user on 2026-11-08, drawing on the Ruby gem `protocol-content` 0.2.0 and the gems it uses. The user decided that every field has a default limit, small for text and larger for uploads, all overridable; that the derive computes a whole-body maximum from the declaration so the parser can refuse a body larger than the struct could accept and the server layer can reject early on `Content-Length`; that enforcement while parsing remains authoritative; that the epilogue, which `base-web-multipart` leaves unlimited, gets a small limit; and that part header sections get a tighter limit than `base-web-multipart`'s 64 KiB, so that the maximum is useful. The `Content-Length` check is the server layer's, and this crate only exposes the maximum.

The user asked that remaining values follow the gem where it has them. The upload and total content limits are `UPLOAD_SIZE_LIMIT` and `TOTAL_SIZE_LIMIT` in `protocol-multipart` 0.7.0's `lib/protocol/multipart/form_data.rb`; the depth limit is `Nested::DEPTH_LIMIT` in `protocol-url` 0.19.0's `lib/protocol/url/form_data/nested.rb`; the pair count and URL-encoded body limits are `PAIR_COUNT_LIMIT` and `SIZE_LIMIT` in its `lib/protocol/url/form_data/parser.rb`; and the part count, header count, and preamble limits are the gem's, in `lib/protocol/multipart/parser.rb`, and `base-web-multipart`'s `MultipartLimits` defaults. The gem's text field limit is 2 MiB (`FIELD_SIZE_LIMIT`), which the user's decision for small text defaults replaces with 64 KiB: that is far more than a text input or ordinary text area holds, matches the family's other small limits, keeps a struct of a dozen text fields to under a megabyte, and a field expecting more, such as an article body, can say so. The gem has no limit on an array's elements, which its part and pair counts bound instead; 128 is the part count limit, so an array cannot by default hold more than a multipart body could. An 8 KiB header section is the per-line limit common HTTP servers apply and comfortably holds a `Content-Disposition` with a long UTF-8 filename and a `Content-Type`. Browsers send nothing after the closing boundary but a line break, so 1 KiB of epilogue is ample.

The gem applies its limits as content arrives, but its declared upload `size_limit` only records a violation and keeps reading (`UploadField#process` in `lib/protocol/content/parameters/field.rb`). This crate is deliberately stricter: an over-limit upload stops parsing. Enforcing the computed maximum as a running total is what lets duplicate names, which `design/parsing.md` resolves as the gem does, coexist with a bound that holds.

## Scope

The declaration's limit attributes and defaults, the computed maximum body sizes, and their enforcement by the parser for every format.

## Validation

Unit tests of the computed maximum for representative declarations, against hand-computed values for each format, including saturation. For each limit, tests that a body at the limit parses and a body one byte or one item over is refused, fed whole and a byte at a time, and that the refusal comes no later than the byte that exceeds it. Tests that a body within every per-field limit but over a running total, for example by repeating a field, is refused, and that no body larger than the computed maximum parses.
