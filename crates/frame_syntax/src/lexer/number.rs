//! Number literals: checking their form and reading their value.
//!
//! The lexer finds where a literal ends; this module tells whether its text is valid. The
//! literal's type is not known yet, so a value too large for every type is not refused here.

use std::ops::Range;

use frame_diagnostics::{Diagnostic, Fix, Prefix, Primary, RuleCode};
use frame_source::{FileId, Span};

use super::{TokenKind, usize_to_u32};
use Defect::{
    FloatNotDecimal, InvalidDigit, LeadingZero, MisplacedUnderscore, MissingDigits, Suffix,
    UppercasePrefix,
};

const INTEGER_LITERAL: RuleCode = RuleCode::new(Prefix::Lex, 13);
const FLOAT_LITERAL: RuleCode = RuleCode::new(Prefix::Lex, 14);

/// The base of an integer literal, given by its prefix. Without a prefix, it is decimal.
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

    /// Returns the base's name with its article, as a title writes it: `an octal`.
    fn name_with_article(self) -> &'static str {
        match self {
            Base::Binary => "a binary",
            Base::Octal => "an octal",
            Base::Decimal => "a decimal",
            Base::Hexadecimal => "a hexadecimal",
        }
    }
}

/// What is wrong with a refused literal.
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
    /// A float is not written in decimal, as in `0x1.5`.
    FloatNotDecimal,
}

/// A refused literal: its first defect, and where it is.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct NumberError {
    defect: Defect,
    /// The defect's bytes, counted from the start of the literal.
    range: Range<usize>,
    /// Whether the literal is a float. It picks the rule, LEX-14 rather than LEX-13, and the
    /// kind of the placeholder token.
    is_float: bool,
}

impl NumberError {
    /// Builds the diagnostic of this defect, for the literal `text` found at byte `start`.
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
            LeadingZero => {
                let decimal = match text.trim_start_matches(['0', '_']) {
                    "" => "0",
                    rest => rest,
                };
                let octal = format!("0o{decimal}");
                // A suggestion must itself be a valid literal: `089` has no octal form, and
                // `007_` keeps a misplaced `_` once its zeros are gone.
                let help = match (validate_number(decimal), validate_number(&octal)) {
                    (Ok(_), Ok(_)) => {
                        format!("write `{decimal}`, or `{octal}` for an octal number")
                    }
                    (Ok(_), Err(_)) => format!("write `{decimal}`"),
                    (Err(_), _) => String::from("remove the leading zeros"),
                };
                Diagnostic::new(
                    code,
                    "number literal with a leading zero",
                    Primary::span(span),
                )
                .with_help(help)
            }
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
                    "invalid digit `{found}` in a {} literal",
                    base.name_with_article()
                ),
                Primary::span(span),
            ),
            MissingDigits => Diagnostic::new(
                code,
                format!("missing digits after `{}`", &text[..2]),
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
            FloatNotDecimal => {
                Diagnostic::new(code, "float literal not in decimal", Primary::span(span))
            }
        }
    }

    /// Returns the token of a refused literal. Its value does not matter, since the program
    /// is refused.
    pub(super) fn placeholder(&self) -> TokenKind {
        if self.is_float {
            TokenKind::Float(String::from("0"))
        } else {
            TokenKind::Int(Some(0))
        }
    }
}

/// Checks that `text` is an integer or a float literal, and returns its kind and value.
///
/// `text` is a whole literal, as the lexer read it, so it holds only ASCII. When the literal
/// is refused, the error holds its first defect from the left.
pub(super) fn validate_number(text: &str) -> Result<TokenKind, NumberError> {
    let bytes = text.as_bytes();

    let (base, uppercase_prefix, digits_start) = read_prefix(bytes);

    // Decided before reading the digits: in `1_e5`, the reading stops at the `_`, but the
    // exponent still makes it a float.
    let is_float = text.contains('.')
        || (base == Base::Decimal && (0..bytes.len()).any(|i| is_exponent_at(bytes, i)));

    if uppercase_prefix {
        return Err(NumberError {
            defect: UppercasePrefix,
            range: 0..2,
            is_float,
        });
    }

    // Its defect starts the literal, so it comes before any other. A float may start with
    // zeros: `007.5` is valid.
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
            // Further on, a letter starts a suffix; right after the prefix, it is a digit.
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

/// Tells whether the literal at the start of `text` is decimal. An uppercase prefix counts as
/// a prefix, so that `0X1e+5` splits as its fix `0x1e+5` does.
pub(super) fn starts_decimal(text: &str) -> bool {
    read_prefix(text.as_bytes()).0 == Base::Decimal
}

/// Returns the value of `digits`, written in `base` without its prefix, or `None` if it does
/// not fit in a `u64`.
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

/// Reads the digits of `base` and the well-placed `_` from `start`, and returns where they
/// stop.
fn scan_digits(
    bytes: &[u8],
    start: usize,
    base: Base,
    is_float: bool,
) -> Result<usize, NumberError> {
    let in_base = |c: u8| char::from(c).is_digit(base.radix());
    // Around a `_`, a decimal digit outside the base still counts as a digit, so that
    // `0b1_2` reports the `2` rather than the `_`.
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

/// Returns the base that the prefix of `bytes` gives, whether that prefix is in uppercase,
/// and where the digits start.
fn read_prefix(bytes: &[u8]) -> (Base, bool, usize) {
    match (bytes.first(), bytes.get(1)) {
        (Some(b'0'), Some(&c @ (b'b' | b'B'))) => (Base::Binary, c.is_ascii_uppercase(), 2),
        (Some(b'0'), Some(&c @ (b'o' | b'O'))) => (Base::Octal, c.is_ascii_uppercase(), 2),
        (Some(b'0'), Some(&c @ (b'x' | b'X'))) => (Base::Hexadecimal, c.is_ascii_uppercase(), 2),
        _ => (Base::Decimal, false, 0),
    }
}

/// Tells whether an exponent starts at `i`: an `e` or an `E`, then a digit, or a sign and a
/// digit.
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
        FloatNotDecimal, InvalidDigit, LeadingZero, MisplacedUnderscore, MissingDigits, Suffix,
        UppercasePrefix,
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
        ]);
    }

    #[test]
    fn integer_too_large_for_u64_has_no_value() {
        assert_validated(&[
            ("18446744073709551615", int(u64::MAX)),
            ("18446744073709551616", Ok(TokenKind::Int(None))),
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
            // `dec_digits` accepts a leading zero.
            ("007.5", float("007.5")),
            ("0e5", float("0e5")),
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
        ]);
    }

    #[test]
    fn digit_outside_the_base_is_invalid() {
        assert_validated(&[
            ("0b102", refused(InvalidDigit(Base::Binary), 4..5, false)),
            ("0o8", refused(InvalidDigit(Base::Octal), 2..3, false)),
            ("0xG", refused(InvalidDigit(Base::Hexadecimal), 2..3, false)),
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
