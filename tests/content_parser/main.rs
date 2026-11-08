// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

mod errors;
mod fields;
mod formatting;
mod limits;
mod round_trip;
mod support;
mod uploads;
mod url_encoded;

use base_web_content::{Content, FieldValue, Upload};
use support::Handle;

#[derive(FieldValue, Debug, PartialEq)]
pub enum Plan {
    #[content(rename = "free")]
    Free,
    #[content(rename = "pro")]
    Pro,
}

#[derive(Content, Debug, PartialEq)]
pub struct Address {
    pub city: String,
    pub postcode: Option<String>,
}

#[derive(Content, Debug, PartialEq)]
pub struct Line {
    pub sku: String,
    pub quantity: Option<u32>,
}

#[derive(Content, Debug)]
pub struct Signup {
    pub name: String,
    #[content(rename = "e-mail")]
    pub email: Option<String>,
    pub age: Option<u8>,
    pub plan: Option<Plan>,
    #[content(count_limit = 3)]
    pub tags: Option<Vec<String>>,
    pub address: Option<Address>,
    #[content(count_limit = 2)]
    pub lines: Option<Vec<Line>>,
    pub secret: Option<Vec<u8>>,
    #[content(accept("image/png", "image/jpeg"), size_limit = 16)]
    pub avatar: Option<Upload<Handle>>,
    #[content(count_limit = 2, size_limit = 16)]
    pub documents: Option<Vec<Upload<Handle>>>,
}
