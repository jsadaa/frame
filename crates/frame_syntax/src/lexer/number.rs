//! Number literals: checking their form and reading their value.
//!
//! The lexer finds where a literal ends, and this module checks its text. We don't know the
//! literal's type yet, so a value too large for every type isn't rejected here.

use std::ops::Range;

use frame_diagnostics::{Diagnostic, Fix, Prefix, Primary, RuleCode};
use frame_source::{FileId, Span};

use super::{TokenKind, usize_to_u32};
use Defect::{
    AfterExponent, FloatNotDecimal, InvalidDigit, LeadingZero, MisplacedUnderscore, MissingDigits,
    Suffix, UppercasePrefix,
};

const INTEGER_LITERAL: RuleCode = RuleCode::new(Prefix::Lex, 13);
const FLOAT_LITERAL: RuleCode = RuleCode::new(Prefix::Lex, 14);

/// An integer literal's base, set by its prefix. No prefix means decimal.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Base {
    Binary,
    Octal,
    Decimal,
    Hexadecimal,
}

impl Base {
    fn radix(self) -> u32 {
        match self {
            Base::Binary => 2,
            Base::Octal => 8,
            Base::Decimal => 10,
            Base::Hexadecimal => 16,
        }
    }

    /// With its article, for diagnostic titles: `an octal`.
    fn name_with_article(self) -> &'static str {
        match self {
            Base::Binary => "a binary",
            Base::Octal => "an octal",
            Base::Decimal => "a decimal",
            Base::Hexadecimal => "a hexadecimal",
        }
    }
}

/// What's wrong with an invalid literal.
#[derive(Debug, PartialEq, Eq)]
enum Defect {
    /// A decimal integer of several digits starts with `0`, as in `007`.
    LeadingZero,
    /// A `_` is not between two digits, as in `1__0`.
    MisplacedUnderscore,
    /// The prefix is in uppercase, as in `0X1F`.
    UppercasePrefix,
    /// A digit is not in the base, as in `0b102`.
    InvalidDigit(Base),
    /// A prefix has no digits after it, as in `0x`.
    MissingDigits,
    /// Letters follow the digits, as in `10i64`.
    Suffix,
    /// A point or a second exponent follows the exponent, as in `1e5.5`.
    AfterExponent,
    /// A float is not written in decimal, as in `0x1.5`.
    FloatNotDecimal,
}

/// An invalid literal: its first defect, and where it is.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct NumberError {
    defect: Defect,
    /// The defect's byte range, relative to the literal's start.
    range: Range<usize>,
    /// Whether the literal is a float. It picks the rule, LEX-14 rather than LEX-13, and the
    /// placeholder's kind.
    is_float: bool,
}

impl NumberError {
    /// `text` is the whole literal, found at byte `start` of the file.
    pub(super) fn to_diagnostic(&self, text: &str, file: FileId, start: usize) -> Diagnostic {
        let code = if self.is_float {
            FLOAT_LITERAL
        } else {
            INTEGER_LITERAL
        };
        let span = Span::new(
            file,
            usize_to_u32(start + self.range.start),
            usize_to_u32(start + self.range.end),
        );
        let found = &text[self.range.clone()];

        match self.defect {
            LeadingZero => Diagnostic::new(
                code,
                "number literal with a leading zero",
                Primary::span(span),
            )
            .with_help(leading_zero_help(text)),
            MisplacedUnderscore => {
                Diagnostic::new(code, "`_` must be between two digits", Primary::span(span))
            }
            UppercasePrefix => Diagnostic::new(
                code,
                format!("uppercase prefix `{found}`"),
                Primary::span(span),
            )
            .with_help("prefixes are lowercase")
            .with_fix(Fix::new(span, found.to_ascii_lowercase())),
            InvalidDigit(base) => Diagnostic::new(
                code,
                format!(
                    "invalid digit `{found}` in {} literal",
                    base.name_with_article()
                ),
                Primary::span(span),
            ),
            MissingDigits => Diagnostic::new(
                code,
                format!("missing digits after `{found}`"),
                Primary::span(span),
            ),
            Suffix => Diagnostic::new(
                code,
                format!("number literal with a suffix `{found}`"),
                Primary::span(span),
            )
            .with_help(format!(
                "remove `{found}`: a literal takes the expected type"
            )),
            AfterExponent => Diagnostic::new(
                code,
                format!("`{found}` after the exponent"),
                Primary::span(span),
            )
            .with_help(format!("remove `{found}`: the exponent ends the literal")),
            FloatNotDecimal => {
                Diagnostic::new(code, "float literal not in decimal", Primary::span(span))
            }
        }
    }

    /// The token an invalid literal produces. Its value doesn't matter: the program is
    /// rejected anyway.
    pub(super) fn placeholder(&self) -> TokenKind {
        if self.is_float {
            TokenKind::Float(String::from("0"))
        } else {
            TokenKind::Int(Some(0))
        }
    }
}

/// Suggests a value only when the leading zero is the only defect. For `007_`, `7` would
/// hide the misplaced `_`, and `7_` is invalid too.
fn leading_zero_help(text: &str) -> String {
    if !is_all_digits(text, Base::Decimal) {
        return String::from("remove the leading zeros");
    }

    let value = match text.trim_start_matches(['0', '_']) {
        "" => "0",
        rest => rest,
    };
    if is_all_digits(value, Base::Octal) {
        format!("write `{value}`, or `0o{value}` for an octal number")
    } else {
        format!("write `{value}`")
    }
}

/// Checks a number literal, and returns its token kind with its value.
///
/// `text` is a whole literal, as the lexer cut it: ASCII only, and any `.` or sign in it is
/// followed by a digit. On error, you get the leftmost defect.
pub(super) fn validate_number(text: &str) -> Result<TokenKind, NumberError> {
    let bytes = text.as_bytes();

    let (base, uppercase_prefix, digits_start) = read_prefix(bytes);

    // Decide this up front: in `1_e5`, the digit scan stops at the `_`, but the exponent
    // still makes it a float.
    let is_float = text.contains('.')
        || (base == Base::Decimal && (0..bytes.len()).any(|i| is_exponent_at(bytes, i)));

    if uppercase_prefix {
        return Err(NumberError {
            defect: UppercasePrefix,
            range: 0..2,
            is_float,
        });
    }

    // A leading zero is always the leftmost defect, so check it first. Floats may start with
    // zeros: `007.5` is fine.
    if base == Base::Decimal && !is_float && bytes.first() == Some(&b'0') {
        let leading_digits = bytes
            .iter()
            .take_while(|&&c| c.is_ascii_digit() || c == b'_')
            .filter(|&&c| c.is_ascii_digit())
            .count();

        if leading_digits >= 2 {
            return Err(NumberError {
                defect: LeadingZero,
                range: 0..bytes.len(),
                is_float,
            });
        }
    }

    if base != Base::Decimal {
        match bytes.get(2) {
            None => {
                return Err(NumberError {
                    defect: MissingDigits,
                    range: 0..bytes.len(),
                    is_float,
                });
            }
            // Right after the prefix, a letter is a bad digit, as in `0oa7`. Further on, it
            // would start a suffix.
            Some(c) if c.is_ascii_alphabetic() && !char::from(*c).is_digit(base.radix()) => {
                return Err(NumberError {
                    defect: InvalidDigit(base),
                    range: 2..3,
                    is_float,
                });
            }
            _ => {}
        }
    }

    let mut end = scan_digits(bytes, digits_start, base, is_float)?;

    if bytes.get(end) == Some(&b'.') {
        if base != Base::Decimal {
            return Err(NumberError {
                defect: FloatNotDecimal,
                range: 0..bytes.len(),
                is_float,
            });
        }
        end = scan_digits(bytes, end + 1, base, is_float)?;
    }

    if base == Base::Decimal && is_exponent_at(bytes, end) {
        end += 1;
        if matches!(bytes.get(end), Some(b'+' | b'-')) {
            end += 1;
        }
        end = scan_digits(bytes, end, base, is_float)?;

        // Not a suffix: unlike the `f32` of `1e5f32`, a point or a second exponent names no
        // type.
        if bytes.get(end) == Some(&b'.') || is_exponent_at(bytes, end) {
            return Err(NumberError {
                defect: AfterExponent,
                range: end..bytes.len(),
                is_float,
            });
        }
    }

    if end < bytes.len() {
        return Err(NumberError {
            defect: Suffix,
            range: end..bytes.len(),
            is_float,
        });
    }

    if is_float {
        Ok(TokenKind::Float(text.replace('_', "")))
    } else {
        Ok(TokenKind::Int(int_value(&text[digits_start..], base)))
    }
}

/// Uppercase prefixes count, so `0X1e+5` splits the same way as its fix, `0x1e+5`.
pub(super) fn starts_decimal(text: &str) -> bool {
    read_prefix(text.as_bytes()).0 == Base::Decimal
}

/// `digits` has no prefix. Returns `None` if the value doesn't fit in a `u64`.
fn int_value(digits: &str, base: Base) -> Option<u64> {
    let radix = u64::from(base.radix());
    let mut value: u64 = 0;

    for c in digits.chars() {
        if c == '_' {
            continue;
        }
        let digit = c
            .to_digit(base.radix())
            .expect("validate_number checked the digits");
        value = value.checked_mul(radix)?.checked_add(u64::from(digit))?;
    }

    Some(value)
}

/// `true` if `text` is only digits of `base`, with each `_` between two of them.
fn is_all_digits(text: &str, base: Base) -> bool {
    scan_digits(text.as_bytes(), 0, base, false).is_ok_and(|end| end == text.len())
}

/// Scans digits of `base` from `start`, `_` included, and returns where they stop.
///
/// Fails on a `_` that isn't between two digits, and on a decimal digit outside `base`.
fn scan_digits(
    bytes: &[u8],
    start: usize,
    base: Base,
    is_float: bool,
) -> Result<usize, NumberError> {
    let in_base = |c: u8| char::from(c).is_digit(base.radix());
    // Around a `_`, any decimal digit counts, even outside the base: `0b1_2` should report
    // the `2`, not the `_`.
    let is_digit = |c: u8| c.is_ascii_digit() || in_base(c);

    for i in start..bytes.len() {
        let c = bytes[i];

        match c {
            c if in_base(c) => {}
            b'_' => {
                if i == start
                    || i + 1 == bytes.len()
                    || !is_digit(bytes[i - 1])
                    || !is_digit(bytes[i + 1])
                {
                    return Err(NumberError {
                        defect: MisplacedUnderscore,
                        range: i..(i + 1),
                        is_float,
                    });
                }
            }
            c if c.is_ascii_digit() => {
                return Err(NumberError {
                    defect: InvalidDigit(base),
                    range: i..(i + 1),
                    is_float,
                });
            }
            _ => return Ok(i),
        }
    }

    Ok(bytes.len())
}

/// Returns the base, whether its prefix is uppercase, and where the digits start.
fn read_prefix(bytes: &[u8]) -> (Base, bool, usize) {
    match (bytes.first(), bytes.get(1)) {
        (Some(b'0'), Some(&c @ (b'b' | b'B'))) => (Base::Binary, c.is_ascii_uppercase(), 2),
        (Some(b'0'), Some(&c @ (b'o' | b'O'))) => (Base::Octal, c.is_ascii_uppercase(), 2),
        (Some(b'0'), Some(&c @ (b'x' | b'X'))) => (Base::Hexadecimal, c.is_ascii_uppercase(), 2),
        _ => (Base::Decimal, false, 0),
    }
}

/// An exponent is `e` or `E`, then a digit, or a sign and a digit.
fn is_exponent_at(bytes: &[u8], i: usize) -> bool {
    if !matches!(bytes.get(i), Some(b'e' | b'E')) {
        return false;
    }

    match bytes.get(i + 1) {
        Some(b'0'..=b'9') => true,
        Some(b'+' | b'-') => bytes.get(i + 2).is_some_and(|&c| c.is_ascii_digit()),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    use super::{Base, Defect, NumberError, validate_number};
    use crate::TokenKind;

    use Defect::{
        AfterExponent, FloatNotDecimal, InvalidDigit, LeadingZero, MisplacedUnderscore,
        MissingDigits, Suffix, UppercasePrefix,
    };

    type Validated = Result<TokenKind, NumberError>;

    fn int(value: u64) -> Validated {
        Ok(TokenKind::Int(Some(value)))
    }

    fn float(text: &str) -> Validated {
        Ok(TokenKind::Float(text.to_string()))
    }

    fn refused(defect: Defect, range: Range<usize>, is_float: bool) -> Validated {
        Err(NumberError {
            defect,
            range,
            is_float,
        })
    }

    /// Validates each text, and compares the result in full.
    fn assert_validated(cases: &[(&str, Validated)]) {
        for (text, expected) in cases {
            assert_eq!(&validate_number(text), expected, "validation of {text:?}");
        }
    }

    #[test]
    fn integers_give_their_value() {
        assert_validated(&[
            ("0", int(0)),
            ("7", int(7)),
            ("1_000_000", int(1_000_000)),
            ("0xFF_FF", int(0xFFFF)),
            ("0xff", int(0xFF)),
            ("0o17", int(0o17)),
            ("0b1010", int(0b1010)),
            // In hexadecimal, `e` is a digit, not an exponent.
            ("0x1e5", int(0x1e5)),
        ]);
    }

    #[test]
    fn integer_too_large_for_u64_has_no_value() {
        assert_validated(&[
            ("18446744073709551615", int(u64::MAX)),
            ("18446744073709551616", Ok(TokenKind::Int(None))),
            // Overflows on the multiplication, not just the addition.
            ("99999999999999999999", Ok(TokenKind::Int(None))),
        ]);
    }

    #[test]
    fn floats_keep_their_text_without_underscores() {
        assert_validated(&[
            ("0.5", float("0.5")),
            ("1.0", float("1.0")),
            ("1e9", float("1e9")),
            ("2.5E-3", float("2.5E-3")),
            ("1e+5", float("1e+5")),
            ("1_000.000_1", float("1000.0001")),
            // Floats may start with zeros.
            ("007.5", float("007.5")),
            ("0e5", float("0e5")),
            ("1e0", float("1e0")),
        ]);
    }

    #[test]
    fn integer_with_a_leading_zero_is_refused() {
        assert_validated(&[("007", refused(LeadingZero, 0..3, false))]);
    }

    #[test]
    fn underscore_must_be_between_two_digits() {
        assert_validated(&[
            ("1__0", refused(MisplacedUnderscore, 1..2, false)),
            ("1_", refused(MisplacedUnderscore, 1..2, false)),
            ("0x_1", refused(MisplacedUnderscore, 2..3, false)),
            ("1_e5", refused(MisplacedUnderscore, 1..2, true)),
        ]);
    }

    #[test]
    fn uppercase_prefix_is_refused() {
        assert_validated(&[
            ("0X1F", refused(UppercasePrefix, 0..2, false)),
            ("0B1", refused(UppercasePrefix, 0..2, false)),
            ("0O7", refused(UppercasePrefix, 0..2, false)),
            // The point makes it a float, so LEX-14 reports it.
            ("0X1.5", refused(UppercasePrefix, 0..2, true)),
        ]);
    }

    #[test]
    fn digit_outside_the_base_is_invalid() {
        assert_validated(&[
            ("0b102", refused(InvalidDigit(Base::Binary), 4..5, false)),
            ("0o8", refused(InvalidDigit(Base::Octal), 2..3, false)),
            ("0xG", refused(InvalidDigit(Base::Hexadecimal), 2..3, false)),
            // Only the digit, not what follows it.
            ("0oa7", refused(InvalidDigit(Base::Octal), 2..3, false)),
            // The `2` is a digit, so the `_` before it is in place.
            ("0b1_2", refused(InvalidDigit(Base::Binary), 4..5, false)),
        ]);
    }

    #[test]
    fn prefix_without_digits_is_refused() {
        assert_validated(&[("0x", refused(MissingDigits, 0..2, false))]);
    }

    #[test]
    fn letter_after_the_digits_starts_a_suffix() {
        assert_validated(&[
            ("10i64", refused(Suffix, 2..5, false)),
            ("1e", refused(Suffix, 1..2, false)),
            ("1.5f32", refused(Suffix, 3..6, true)),
            ("0x1z", refused(Suffix, 3..4, false)),
            ("0b1a", refused(Suffix, 3..4, false)),
            // Only a decimal literal has an exponent.
            ("0b1e5", refused(Suffix, 3..5, false)),
            // `0d` is not a prefix.
            ("0d34", refused(Suffix, 1..4, false)),
            // After an exponent, letters still make a suffix.
            ("1e5f32", refused(Suffix, 3..6, true)),
            // An `e` with no digit after it is a letter, not a second exponent.
            ("1e5e", refused(Suffix, 3..4, true)),
        ]);
    }

    #[test]
    fn point_or_exponent_after_the_exponent_is_refused() {
        assert_validated(&[
            ("1e5.5", refused(AfterExponent, 3..5, true)),
            ("1e5e5", refused(AfterExponent, 3..5, true)),
            ("1e+5e+5", refused(AfterExponent, 4..7, true)),
        ]);
    }

    #[test]
    fn float_must_be_decimal() {
        assert_validated(&[("0x1.5", refused(FloatNotDecimal, 0..5, true))]);
    }

    #[test]
    fn only_the_first_defect_is_reported() {
        assert_validated(&[("0X1__0", refused(UppercasePrefix, 0..2, false))]);
    }
}
