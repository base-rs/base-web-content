// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use std::fmt;

/// A field's value converted from its submitted text, the conversion trait for scalar fields.
///
/// `String`, the integer types, `f32`, `f64`, and `bool` implement it, as does an enumeration with `#[derive(FieldValue)]`. Implement it for a type of your own to accept that type as a field, with the text converted by your code: a failure is an `invalid_type` violation, like any other.
///
/// The text is valid UTF-8, whatever character set the part declared, since text in another character set is refused before it is converted.
///
/// ```
/// use base_web_content::{FieldValue, FieldValueError};
///
/// /// A colour written as `#rrggbb`.
/// struct Colour(u32);
///
/// impl FieldValue for Colour {
///     fn from_field_text(text: &str) -> Result<Self, FieldValueError> {
///         let digits = text.strip_prefix('#').filter(|digits| digits.len() == 6 && digits.bytes().all(|byte| byte.is_ascii_hexdigit()));
///         let digits = digits.ok_or(FieldValueError)?;
///         u32::from_str_radix(digits, 16).map(Colour).map_err(|_| FieldValueError)
///     }
/// }
///
/// assert_eq!(Colour::from_field_text("#ff8000").map(|colour| colour.0), Ok(0xff8000));
/// assert!(Colour::from_field_text("orange").is_err());
/// ```
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be converted from a field's text",
    note = "implement `FieldValue` for it, or derive it for an enumeration"
)]
pub trait FieldValue: Sized + 'static {
    /// The value the text spells.
    ///
    /// # Errors
    ///
    /// [`FieldValueError`] when the text does not spell a value of this type.
    fn from_field_text(text: &str) -> Result<Self, FieldValueError>;
}

/// Why a field's text could not be converted to its type. It carries no detail, since the text may be a password or personal data that must not reach logs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FieldValueError;

impl fmt::Display for FieldValueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("invalid field value")
    }
}

impl std::error::Error for FieldValueError {}

// SECURITY: Text may contain any character UTF-8 allows, including NUL, CR, LF, and bidirectional controls, sent raw in multipart content or escaped in URL-encoded content. This is not an injection weakness here: the parser interprets none of it, and an application that writes a value into another syntax, such as a header, a log line, or a path, must encode or check it for that syntax.
/// Any text, unchanged.
impl FieldValue for String {
    fn from_field_text(text: &str) -> Result<Self, FieldValueError> {
        Ok(text.to_owned())
    }
}

/// Exactly `true` or `false`.
impl FieldValue for bool {
    fn from_field_text(text: &str) -> Result<Self, FieldValueError> {
        match text {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(FieldValueError),
        }
    }
}

/// The digits of an integer: an optional sign followed by one or more ASCII decimal digits, as `(negative, digits)`.
fn integer_digits(text: &str) -> Option<(bool, &str)> {
    let (negative, digits) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };
    (!digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()))
        .then_some((negative, digits))
}

macro_rules! signed_field_values {
    ($($type:ty),*) => {$(
        /// An optional `+` or `-` followed by one or more ASCII decimal digits, within the type's range.
        impl FieldValue for $type {
            fn from_field_text(text: &str) -> Result<Self, FieldValueError> {
                // The standard library accepts exactly this syntax for signed integers, once the text is known not to be a bare sign.
                integer_digits(text).ok_or(FieldValueError)?;
                text.parse().map_err(|_| FieldValueError)
            }
        }
    )*};
}

macro_rules! unsigned_field_values {
    ($($type:ty),*) => {$(
        /// An optional `+` or `-` followed by one or more ASCII decimal digits, within the type's range, so `-0` is zero.
        impl FieldValue for $type {
            fn from_field_text(text: &str) -> Result<Self, FieldValueError> {
                let (negative, digits) = integer_digits(text).ok_or(FieldValueError)?;
                // A negative value is out of range unless it is zero, which the standard library would refuse for its sign.
                if negative {
                    return if digits.bytes().all(|byte| byte == b'0') { Ok(0) } else { Err(FieldValueError) };
                }
                digits.parse().map_err(|_| FieldValueError)
            }
        }
    )*};
}

signed_field_values!(i8, i16, i32, i64, i128, isize);
unsigned_field_values!(u8, u16, u32, u64, u128, usize);

/// Whether `text` is a decimal number: an optional sign, digits with an optional fraction, or a fraction alone, and an optional exponent, such as `-1.5`, `.5`, `5.`, or `1e3`.
fn is_decimal(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut index = usize::from(matches!(bytes.first(), Some(b'+' | b'-')));
    let digits = |index: &mut usize| {
        let start = *index;
        while bytes.get(*index).is_some_and(u8::is_ascii_digit) {
            *index += 1;
        }
        *index - start
    };
    let mut mantissa = digits(&mut index);
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        mantissa += digits(&mut index);
    }
    if mantissa == 0 {
        return false;
    }
    if matches!(bytes.get(index), Some(b'e' | b'E')) {
        index += 1;
        index += usize::from(matches!(bytes.get(index), Some(b'+' | b'-')));
        if digits(&mut index) == 0 {
            return false;
        }
    }
    index == bytes.len()
}

macro_rules! float_field_values {
    ($($type:ty),*) => {$(
        /// A finite decimal number with an optional sign, fraction, and exponent. Whitespace, `_` separators, hexadecimal, and `NaN` or infinity in any spelling, including a number too large for the type, are refused.
        impl FieldValue for $type {
            fn from_field_text(text: &str) -> Result<Self, FieldValueError> {
                if !is_decimal(text) {
                    return Err(FieldValueError);
                }
                // The syntax is a subset of what the standard library parses, so only the range can fail, as infinity.
                let value: $type = text.parse().expect("a decimal number parses");
                if value.is_finite() { Ok(value) } else { Err(FieldValueError) }
            }
        }
    )*};
}

float_field_values!(f32, f64);

/// The character set a part declared for its text: none, which means UTF-8 for a form, UTF-8, US-ASCII, or another, which is not supported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Charset {
    Unspecified,
    Utf8,
    UsAscii,
    Unsupported,
}

impl Charset {
    /// The character set named by a `charset` parameter, if there is one.
    pub(crate) fn from_parameter(charset: Option<&str>) -> Self {
        match charset {
            None => Self::Unspecified,
            Some(name) if name.eq_ignore_ascii_case("utf-8") => Self::Utf8,
            Some(name) if name.eq_ignore_ascii_case("us-ascii") => Self::UsAscii,
            Some(_) => Self::Unsupported,
        }
    }

    /// The bytes as text in this character set, or `None` when they are not valid in it or it is not supported.
    pub(crate) fn decode(self, bytes: Vec<u8>) -> Option<String> {
        match self {
            Self::Unsupported => None,
            Self::UsAscii if !bytes.is_ascii() => None,
            _ => String::from_utf8(bytes).ok(),
        }
    }
}
