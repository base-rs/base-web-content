// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_web_content::Content;

#[derive(Content)]
struct Profile {
    #[content(maximum = 10)]
    name: String,
}

fn main() {}
