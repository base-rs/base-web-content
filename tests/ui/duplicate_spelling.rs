// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_web_content::FieldValue;

#[derive(FieldValue)]
enum Role {
    Admin,
    #[content(rename = "Admin")]
    Administrator,
}

fn main() {}
