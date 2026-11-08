# Declarations

## Requirement

An application must declare the content it accepts by deriving `Content` on a struct with named fields. Each field must declare one form field or nested group, named by its Rust identifier unless a `rename` attribute gives the submitted name, so that any name a form can submit can be declared. The derive must reject, at compile time, a field type it cannot parse, two fields with the same submitted name, and a declaration whose names would be deeper than its depth limit.

Field types must mean:

- `String`: text, which must be valid UTF-8.
- `Vec<u8>`: the submitted bytes, unchanged. `Vec<u8>` always means bytes, never an array of integers. URL-encoded content can only submit UTF-8, as `design/errors.md` describes.
- Integer types (`i8` to `i128`, `u8` to `u128`, and `isize` and `usize`): an optional `+` or `-` followed by one or more ASCII decimal digits, within the type's range. Whitespace, `_` separators, and prefixes such as `0x` must be refused.
- `f32` and `f64`: a decimal number with an optional sign, fraction, and exponent, such as `-1.5`, `.5`, `5.`, or `1e3`. Whitespace, `_` separators, hexadecimal, and `NaN` or infinity in any spelling must be refused.
- `bool`: exactly `true` or `false`.
- An enumeration the application declares with `#[derive(FieldValue)]`: an exact, case-sensitive match of the submitted text against the spellings it lists, each mapped to a value. A variant is spelled as its identifier unless a `rename` attribute gives the spelling, and the derive must reject two variants with the same spelling, an enumeration with no variants or a variant with fields, and a generic enumeration.
- A type implementing the crate's conversion trait, `FieldValue`: converted from the text by the application's code, whose failure is a violation like any other.
- `Upload`: a file. A multipart part is an upload if and only if its `Content-Disposition` has a `filename` parameter, even an empty one.
- A struct that derives `Content`: a nested group, whose fields are submitted as `name[field]`.
- `Option<T>`, for any `T` here other than another `Option`: a field the content may omit.
- `Vec<T>`, for `T` a scalar type other than `u8`, `Upload`, or a nested group: an array, submitted as `name[]`, holding the elements in the order they arrive. An array of nested groups is submitted as `name[][field]`, and an element ends when a field it already has arrives again, which begins the next element. Only the field named directly after `[]` is checked, so `items[][tags][]` begins an element whenever the last one already has `tags`.

The derive reads `Option` and `Vec` from how the field's type is spelled, as `base-message`'s `FieldSection` derive does, so `Option<Vec<T>>` is an array the content may omit, an array's elements cannot be `Option` or `Vec` other than `Vec<u8>`, and `Vec<u8>` is bytes however the path to `u8` is written, though an alias of `u8` is not seen through. A struct that contains itself, by its name or as `Self`, must be rejected, since its names would have no depth, and one that contains itself through another struct fails to compile as a cycle in their declarations; a generic struct, or one without named fields, must be rejected too.

A field is required unless its type is `Option<T>`. A required scalar or upload must be present; a required array must have at least one element; a required nested group must receive at least one of its fields, and its own fields are then checked. `Option<T>` is `None` when nothing for the field arrives. There is no separate nullable declaration: a URL-encoded pair without `=`, which has no value, is a type violation for every field type.

Names must have the form `base`, `base[key]`, or `base[]`, repeated to any depth up to the depth limit, where `base` and each `key` are non-empty and contain no brackets, as `base-web-url`'s `FormPath::parse` reads them for both formats, after decoding, so that the two formats cannot read a name differently; a malformed name is refused as one, however deep. A name is matched against the declaration exactly: `items[0]` does not name an element of `items`, and a scalar name does not match an array declared as `name[]`. The depth limit, the number of components in a name, defaults to 8 and may be overridden on the struct.

The derive must accept attributes for the limits in `design/limits.md`: `size_limit` on any scalar, upload, or array field, `count_limit` on an array, and `accept` on an upload, a set of media ranges such as `image/*` read with `base-web-media-type`'s `MediaRangeSet`. A struct may restrict the formats it accepts; otherwise it accepts every supported format. The attributes are written `#[content(...)]`:

- on a field, `rename = "name"`, `size_limit = N`, `count_limit = N`, and `accept("image/png", "image/*")`, where the media ranges must be valid and without parameters, which `MediaRangeSet` ignores, and are checked when the declaration is compiled, as is that `size_limit` is not on a nested group and `accept` is only on an upload;
- on a struct, `formats("multipart/form-data", "application/x-www-form-urlencoded")`, and the struct's limits from `design/limits.md`: `depth_limit`, `content_size_limit`, `part_count_limit`, `header_size_limit`, `header_count_limit`, `preamble_size_limit`, `epilogue_size_limit`, `pair_count_limit`, and `url_encoded_size_limit`, which apply when the struct is the top level of a body;
- on an enumeration's variant, `rename = "spelling"`.

Each limit is a constant expression of type `usize`, and each attribute may be given once. The `ParameterRules` for reading part names and filenames must be chosen from the request's media type with `base_web_multipart::ParameterRules::for_media_type`, which gives `FormData` for `multipart/form-data` and so refuses `filename*`, unless the caller overrides them.

## Rationale

These decisions were made with the user on 2026-11-08, drawing on the Ruby gem `protocol-content` 0.2.0 and its parameters guide (`guides/parameters/readme.md`, read from the gem's v0.2.0 tag, since the installed gem does not include its guides). The user chose a derive over a builder, nested structs as groups, `Option<T>` for optional fields, `Vec<T>` for arrays, an `Upload` type, attributes for limits and accepted media types, `String` as UTF-8 only with `Vec<u8>` for raw bytes, and `ParameterRules::for_media_type` with an override.

The user then asked that what remained ambiguous follow the gem's implementation:

- Name syntax and arrays follow `protocol-url` 0.19.0, which the gem uses: `Encoding.split` and `Encoding.assign` in `lib/protocol/url/encoding.rb` read `user[name]` as a nested key and `items[]` as an array, and begin a new element of `items[][name]` when its last element already has `name`. An index such as `items[0]` is a hash key there, which an array declaration rejects (`ArrayField#apply` in `lib/protocol/content/parameters/field.rb`). A multiple upload must be named `pictures[]` (`UploadField#upload_field`).
- The depth limit of 8 is `Nested::DEPTH_LIMIT` in `lib/protocol/url/form_data/nested.rb`.
- Required and nullable follow `Model#apply` and `ValueField#apply`: a required field must be present, and a missing value is a type error unless the field is nullable. In a form, only a URL-encoded pair without `=` lacks a value (`Parser#yield_pair` in `lib/protocol/url/form_data/parser.rb`), so no nullable declaration is offered until JSON, whose `null` needs one. A required array of uploads needs at least one upload (`UploadField#apply_multiple`); forms cannot submit an empty array, so the same rule applies to every array.
- Conversions follow the gem's `TYPES` in `lib/protocol/content/parameters.rb`, which uses Ruby's `Integer(value, 10)` and `Float(value)`, and its exact `Enumeration` in `lib/protocol/content/parameters/enumeration.rb`; custom converters follow the gem's callable types (`Builder#resolve`). The gem has no boolean type, and its guide converts `true` and `false` with an enumeration, which `bool` follows. This crate is deliberately stricter than Ruby's conversions, which accept surrounding whitespace, `_` separators, and, for floats, hexadecimal: those leniencies let two readers disagree about a value and are not needed by forms.
- An empty `filename` makes an upload, as in `protocol-multipart` 0.7.0's `lib/protocol/multipart/form_data/parser.rb`, where any `filename` parameter does; browsers send one, with empty content, for a file input left empty.

Matching names exactly, instead of reading `tags` for an array named `tags[]` and reporting a type error afterwards as the gem does, follows from the user's decision that undeclared names are refused as they arrive, in `design/parsing.md`.

The attribute syntax, the `FieldValue` derive and its default spelling, refusing generic structs, and reading containers from the type's spelling were proposed during implementation, where the requirements above left them open, and accepted on 2026-11-09, under the user's delegation of the remaining design decisions. Spelling a variant as its identifier follows the rule that a field is named by its identifier; the gem's `Enumeration` takes explicit spellings, which `rename` gives. Checking only the field after `[]` when deciding whether to begin an element is `Encoding.assign`'s `last.include?(nested)`. Reading names with `FormPath::parse`, which reads the whole name before counting its components, means a name that is malformed is refused as malformed (400) even when it is also too deep; the gem's `Nested` also splits a name before checking its depth. A generic struct is rejected because a declaration is a constant of its type, from which the derive computes its depth and limits when it compiles, and a generic enumeration because an enumeration of unit variants has no use for parameters.

## Scope

`#[derive(Content)]` in `base-web-content-macros`, the field types and conversion trait it accepts, `Upload`, and the declaration attributes.

## Validation

Compile-fail tests (for example with `trybuild`) for unsupported field types, duplicate names, and declarations deeper than the depth limit. Tests for each field type's accepted and refused text, including the leniencies listed above, the range of each integer type, empty and value-less pairs, nested groups, arrays of scalars, uploads, and nested groups with element boundaries, and names that differ from a declaration only in shape. Check that `ParameterRules` follow the media type and the override.
