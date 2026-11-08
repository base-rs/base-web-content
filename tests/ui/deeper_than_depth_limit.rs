// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_web_content::Content;

#[derive(Content)]
struct Address {
    city: String,
}

#[derive(Content)]
#[content(depth_limit = 2)]
struct Person {
    addresses: Vec<Address>,
}

fn main() {}
