// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::{content, field_value};
use syn::DeriveInput;

fn content(source: &str) -> Result<String, String> {
    let input: DeriveInput = syn::parse_str(source).expect("the test input is an item");
    content::expand(&input)
        .map(|tokens| tokens.to_string())
        .map_err(|error| error.to_string())
}

fn field_value(source: &str) -> Result<String, String> {
    let input: DeriveInput = syn::parse_str(source).expect("the test input is an item");
    field_value::expand(&input)
        .map(|tokens| tokens.to_string())
        .map_err(|error| error.to_string())
}

#[track_caller]
fn refuses(result: Result<String, String>, message: &str) {
    assert_eq!(result.expect_err("the declaration is refused"), message);
}

#[test]
fn declares_every_kind_of_field() {
    let expanded = content(
        r#"
        #[doc = "A declaration."]
        #[content(depth_limit = 4, formats("multipart/form-data", "application/x-www-form-urlencoded"))]
        struct Everything {
            #[doc = "Ignored."]
            text: String,
            r#type: Option<String>,
            bytes: Vec<u8>,
            optional_bytes: Option<Vec<u8>>,
            #[content(count_limit = 2, size_limit = 10)]
            byte_arrays: Vec<Vec<u8>>,
            #[content(count_limit = 2)]
            optional_array: Option<Vec<i32>>,
            #[content(rename = "e-mail")]
            email: ::std::string::String,
            #[content(accept("IMAGE/PNG", "image/*"))]
            upload: Upload<()>,
            reference: &'static str,
            pair: (String, String),
            two: Option<String, String>,
            lifetime: Option<'static>,
            qualified: Vec<<X as Y>::u8>,
            primitive: Vec<core::primitive::u8>,
            generic_byte: Vec<u8<X>>,
            tuples: Vec<(u8, u8)>,
        }
        "#,
    )
    .expect("the declaration is accepted");
    for expected in [
        "name : \"type\"",
        "name : \"e-mail\"",
        "Container :: Array (2)",
        "Container :: OptionalArray (2)",
        "Container :: Array (__content :: COUNT_LIMIT)",
        "accept : & [\"image/png\" , \"image/*\"]",
        "depth_limit : 4 ,",
        "multipart_form_data : true , url_encoded : true",
        "__content :: optional_array :: < __H , i32 >",
        "__content :: required :: < __H , Vec < u8 > >",
        "__content :: optional :: < __H , Vec < u8 > >",
        "__content :: array :: < __H , Vec < u8 > >",
        "__content :: required :: < __H , Option < String , String > >",
        "__content :: array :: < __H , < X as Y > :: u8 >",
        "__content :: required :: < __H , Vec < core :: primitive :: u8 > >",
        "__content :: array :: < __H , u8 < X > >",
        "__content :: array :: < __H , (u8 , u8) >",
    ] {
        assert!(expanded.contains(expected), "{expected} in {expanded}");
    }
}

#[test]
fn restricts_formats() {
    let expanded =
        content(r#"#[content(formats("application/x-www-form-urlencoded"))] struct Query {}"#)
            .unwrap();
    assert!(expanded.contains("multipart_form_data : false , url_encoded : true"));
    let expanded = content(r#"#[content(formats("multipart/form-data"))] struct Form {}"#).unwrap();
    assert!(expanded.contains("multipart_form_data : true , url_encoded : false"));
}

#[test]
fn refuses_what_is_not_a_struct_with_named_fields() {
    let message = "Content can only be derived for a struct with named fields";
    refuses(content("struct Name(String);"), message);
    refuses(content("struct Unit;"), message);
    refuses(content("enum Choice { A }"), message);
    refuses(
        content("struct Generic<T> { value: T }"),
        "Content cannot be derived for a generic struct",
    );
    refuses(
        content("struct Constrained where String: Clone { value: String }"),
        "Content cannot be derived for a generic struct",
    );
}

#[test]
fn refuses_duplicate_and_recursive_fields() {
    refuses(
        content(
            r#"struct Signup { email: String, #[content(rename = "email")] confirmation: String }"#,
        ),
        "the name `email` is already declared by the field `email`",
    );
    let message = "a declaration cannot contain itself, since its names would have no depth limit";
    refuses(content("struct Node { children: Vec<Node> }"), message);
    refuses(content("struct Node { parent: Option<Self> }"), message);
}

#[test]
fn refuses_containers_that_cannot_be_submitted() {
    refuses(
        content("struct Profile { nickname: Option<Option<String>> }"),
        "a field is optional once: `Option` cannot contain another `Option`",
    );
    refuses(
        content("struct Profile { tags: Vec<Option<String>> }"),
        "an array's elements cannot be optional",
    );
    refuses(
        content("struct Profile { tags: Option<Vec<Vec<String>>> }"),
        "an array's elements cannot be arrays",
    );
    refuses(
        content("struct Profile { #[content(count_limit = 2)] name: String }"),
        "`count_limit` applies to an array, a `Vec` field",
    );
}

#[test]
fn checks_field_attributes() {
    let field = |attribute: &str| {
        content(&format!(
            "struct Profile {{ #[content({attribute})] name: Upload<()> }}"
        ))
    };
    refuses(
        field("maximum = 1"),
        "unknown `content` attribute; expected `rename`, `size_limit`, `count_limit`, or `accept`",
    );
    refuses(field("a::b = 1"), "expected this path to be an identifier");
    refuses(
        field(r#"rename = "a", rename = "b""#),
        "this attribute is given more than once",
    );
    refuses(
        field("size_limit = 1, size_limit = 2"),
        "this attribute is given more than once",
    );
    refuses(field("rename = 1"), "expected string literal");
    refuses(field("size_limit"), "expected `=`");
    refuses(
        field("size_limit = "),
        "unexpected end of input, expected an expression",
    );
    refuses(field("rename"), "expected `=`");
    refuses(field("count_limit"), "expected `=`");
    refuses(
        field("count_limit = "),
        "unexpected end of input, expected an expression",
    );
    let name = "a field's name must be non-empty and contain no brackets; nested groups and arrays add them";
    refuses(field(r#"rename = """#), name);
    refuses(field(r#"rename = "a[b]""#), name);
    refuses(field(r#"rename = "a]""#), name);
    refuses(
        field(r#"accept()"#),
        "this attribute needs at least one value",
    );
    refuses(field(r#"accept = "image/png""#), "expected parentheses");
    refuses(
        field(r#"accept("image")"#),
        "`accept` needs media ranges, such as `image/png` or `image/*`",
    );
    refuses(
        field(r#"accept("text/plain; charset=utf-8")"#),
        "`accept` matches media types without their parameters, so a range cannot have any",
    );
    refuses(field(r#"accept(1)"#), "expected string literal");
    assert!(field(r#"accept("image/png",)"#).is_ok());
    let array =
        content("struct Profile { #[content(count_limit = 0)] tags: Vec<String> }").unwrap();
    assert!(array.contains("`count_limit` on `tags` must be at least 1"));
    let checked = field(r#"size_limit = 1, accept("image/png")"#).unwrap();
    assert!(
        checked.contains(
            "`size_limit` on `name` applies to a scalar or an upload, not a nested group"
        )
    );
    assert!(checked.contains("`accept` on `name` applies to an upload"));
}

#[test]
fn checks_struct_attributes() {
    let declaration =
        |attribute: &str| content(&format!("#[content({attribute})] struct Profile {{}}"));
    assert!(declaration("depth_limit = 2, epilogue_size_limit = 1 << 20").is_ok());
    refuses(
        declaration("maximum = 1"),
        "unknown `content` attribute; expected `formats` or one of depth_limit, content_size_limit, part_count_limit, header_size_limit, header_count_limit, preamble_size_limit, epilogue_size_limit, pair_count_limit, url_encoded_size_limit",
    );
    refuses(
        declaration("a::depth_limit = 1"),
        "unknown `content` attribute; expected `formats` or one of depth_limit, content_size_limit, part_count_limit, header_size_limit, header_count_limit, preamble_size_limit, epilogue_size_limit, pair_count_limit, url_encoded_size_limit",
    );
    refuses(
        declaration("depth_limit = 1, depth_limit = 2"),
        "this attribute is given more than once",
    );
    refuses(
        declaration("depth_limit = "),
        "unexpected end of input, expected an expression",
    );
    refuses(declaration("depth_limit"), "expected `=`");
    refuses(
        declaration("formats()"),
        "this attribute needs at least one value",
    );
    refuses(
        declaration(r#"formats("application/json")"#),
        "unsupported format; expected `multipart/form-data` or `application/x-www-form-urlencoded`",
    );
    refuses(
        declaration(r#"formats("multipart/form-data", "multipart/form-data")"#),
        "this format is listed twice",
    );
    refuses(
        declaration(r#"formats("multipart/form-data"), formats("multipart/form-data")"#),
        "this attribute is given more than once",
    );
}

#[test]
fn converts_enumerations() {
    let expanded = field_value(
        r#"enum Role { Admin, #[doc = "Ignored."] #[content(rename = "editor")] Editor, r#Type }"#,
    )
    .expect("the enumeration is accepted");
    for expected in [
        "\"Admin\" =>",
        "\"editor\" =>",
        "\"Type\" =>",
        "Self :: Admin",
        "Self :: Editor",
    ] {
        assert!(expanded.contains(expected), "{expected} in {expanded}");
    }
}

#[test]
fn refuses_enumerations_that_cannot_be_spelled() {
    let message = "FieldValue can only be derived for an enumeration of unit variants";
    refuses(field_value("struct Role { name: String }"), message);
    refuses(field_value("enum Shape { Circle(f64) }"), message);
    refuses(field_value("enum Shape { Square { side: f64 } }"), message);
    refuses(
        field_value("enum Never {}"),
        "FieldValue needs at least one variant to convert to",
    );
    refuses(
        field_value("enum Choice<T> { A }"),
        "FieldValue cannot be derived for a generic enumeration",
    );
    refuses(
        field_value("enum Choice where String: Clone { A }"),
        "FieldValue cannot be derived for a generic enumeration",
    );
    refuses(
        field_value(
            r#"#[doc = "Roles."] #[content(rename_all = "lowercase")] enum Role { Admin }"#,
        ),
        "an enumeration takes no `content` attributes; give a variant's spelling with `#[content(rename = \"...\")]` on the variant",
    );
    refuses(
        field_value(r#"enum Role { Admin, #[content(rename = "Admin")] Administrator }"#),
        "the spelling `Admin` is already used by another variant",
    );
    refuses(
        field_value(r#"enum Role { #[content(spelling = "a")] Admin }"#),
        "unknown `content` attribute; expected `rename`",
    );
    refuses(
        field_value("enum Role { #[content(rename)] Admin }"),
        "expected `=`",
    );
    refuses(
        field_value("enum Role { #[content(rename = 1)] Admin }"),
        "expected string literal",
    );
    refuses(
        field_value(r#"enum Role { #[content(rename = "a", rename = "b")] Admin }"#),
        "this attribute is given more than once",
    );
}
