// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_web_content::Content;

#[derive(Content)]
#[content(formats("application/json"))]
struct Profile {
    name: String,
}

fn main() {}
