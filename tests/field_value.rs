// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_web_content::{FieldValue, FieldValueError};
use std::fmt::Debug;

fn accepts<T: FieldValue + PartialEq + Debug>(text: &str, expected: T) {
    assert_eq!(T::from_field_text(text), Ok(expected), "{text:?}");
}

fn refuses<T: FieldValue + Debug>(texts: &[&str]) {
    for text in texts {
        assert_eq!(
            T::from_field_text(text).map(|_| ()),
            Err(FieldValueError),
            "{text:?}"
        );
    }
}

/// Text that no number type accepts: whitespace, separators, prefixes, and malformed signs and digits.
const NOT_NUMBERS: &[&str] = &[
    "", " 1", "1 ", "1\n", "1_000", "0x10", "0b1", "0o7", "+", "-", "+-1", "--1", "1+", "１", "1,0",
];

#[test]
fn text_is_any_string() {
    accepts("", String::new());
    accepts(" café\r\n", " café\r\n".to_owned());
}

#[test]
fn booleans_are_exactly_true_or_false() {
    accepts("true", true);
    accepts("false", false);
    refuses::<bool>(&["", "True", "TRUE", "1", "0", "yes", "on", " true", "false "]);
}

macro_rules! integer_tests {
    ($($name:ident: $type:ty,)*) => {$(
        #[test]
        fn $name() {
            accepts("0", 0 as $type);
            accepts("+7", 7 as $type);
            accepts("-0", 0 as $type);
            accepts("007", 7 as $type);
            accepts(&<$type>::MAX.to_string(), <$type>::MAX);
            accepts(&<$type>::MIN.to_string(), <$type>::MIN);
            accepts(&format!("+{}", <$type>::MAX), <$type>::MAX);
            let over = (<$type>::MAX as i128 as u128 + 1).to_string();
            refuses::<$type>(&[&over, "1.0", "1e3", "-", "١"]);
            refuses::<$type>(NOT_NUMBERS);
            if <$type>::MIN == 0 {
                refuses::<$type>(&["-1", "-00001"]);
                accepts("-000", 0 as $type);
            } else {
                let under = (<$type>::MIN as i128 - 1).to_string();
                refuses::<$type>(&[&under]);
                accepts("-1", (0 as $type).wrapping_sub(1));
            }
        }
    )*};
}

integer_tests! {
    i8_range: i8,
    i16_range: i16,
    i32_range: i32,
    i64_range: i64,
    isize_range: isize,
    u8_range: u8,
    u16_range: u16,
    u32_range: u32,
    u64_range: u64,
    usize_range: usize,
}

#[test]
fn integers_of_128_bits() {
    accepts(&i128::MAX.to_string(), i128::MAX);
    accepts(&i128::MIN.to_string(), i128::MIN);
    refuses::<i128>(&[
        "170141183460469231731687303715884105728",
        "-170141183460469231731687303715884105729",
    ]);
    refuses::<i128>(NOT_NUMBERS);
    accepts(&u128::MAX.to_string(), u128::MAX);
    accepts("-0", 0u128);
    refuses::<u128>(&["340282366920938463463374607431768211456", "-1"]);
    refuses::<u128>(NOT_NUMBERS);
}

#[test]
fn floats_are_finite_decimal_numbers() {
    for (text, expected) in [
        ("0", 0.0),
        ("-1.5", -1.5),
        ("+2", 2.0),
        (".5", 0.5),
        ("5.", 5.0),
        ("1e3", 1000.0),
        ("1E-3", 0.001),
        ("-2.5e+2", -250.0),
        ("1e-400", 0.0),
    ] {
        accepts(text, expected);
        accepts(text, expected as f32);
    }
    let not_floats = [
        ".",
        "e3",
        "1e",
        "1e+",
        "1.5.2",
        "1..5",
        "NaN",
        "nan",
        "inf",
        "+inf",
        "-Infinity",
        "infinity",
        "1e400",
        "0x1p3",
        "1.5f",
    ];
    refuses::<f64>(&not_floats);
    refuses::<f64>(NOT_NUMBERS);
    refuses::<f32>(&["1e39", "3.5e38"]);
    refuses::<f32>(NOT_NUMBERS);
    accepts("3.4e38", 3.4e38f32);
}

#[test]
fn the_error_has_no_detail() {
    assert_eq!(FieldValueError.to_string(), "invalid field value");
}

mod enumerations {
    use super::{accepts, refuses};
    use base_web_content::FieldValue;

    #[derive(FieldValue, Debug, PartialEq)]
    enum Size {
        Small,
        #[content(rename = "medium")]
        Medium,
        #[content(rename = "")]
        Unknown,
        r#Large,
    }

    #[test]
    fn match_the_spellings_exactly() {
        accepts("Small", Size::Small);
        accepts("medium", Size::Medium);
        accepts("", Size::Unknown);
        accepts("Large", Size::Large);
        refuses::<Size>(&["small", "Medium", " Small", "Small ", "r#Large", "SMALL"]);
    }
}
