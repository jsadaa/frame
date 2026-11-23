use std::ops::Range;

use frame_diagnostics::Diagnostic;
use frame_source::FileId;

use super::TokenKind;
use Defect::{
    FloatNotDecimal, InvalidDigit, LeadingZero, MisplacedUnderscore, MissingDigits, Suffix,
    UppercasePrefix,
};

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

    fn name(self) -> &'static str {
        match self {
            Base::Binary => "binary",
            Base::Octal => "octal",
            Base::Decimal => "decimal",
            Base::Hexadecimal => "hexadecimal",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Defect {
    LeadingZero,
    MisplacedUnderscore,
    UppercasePrefix,
    InvalidDigit(Base),
    MissingDigits,
    Suffix,
    FloatNotDecimal,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct NumberError {
    defect: Defect,
    range: Range<usize>, // in bytes, relative to the literal
    is_float: bool,
}

pub(super) fn validate_number(text: &str) -> Result<TokenKind, NumberError> {
    let bytes = text.as_bytes();

    let (base, uppercase_prefix, digits_start) = match (bytes.first(), bytes.get(1)) {
        (Some(b'0'), Some(&c @ (b'b' | b'B'))) => (Base::Binary, c.is_ascii_uppercase(), 2),
        (Some(b'0'), Some(&c @ (b'o' | b'O'))) => (Base::Octal, c.is_ascii_uppercase(), 2),
        (Some(b'0'), Some(&c @ (b'x' | b'X'))) => (Base::Hexadecimal, c.is_ascii_uppercase(), 2),
        _ => (Base::Decimal, false, 0),
    };

    let is_float = text.contains('.')
        || (base == Base::Decimal && (0..bytes.len()).any(|i| is_exponent_at(bytes, i)));

    if uppercase_prefix {
        return Err(NumberError {
            defect: UppercasePrefix,
            range: 0..2,
            is_float,
        });
    }

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

fn int_value(digits: &str, base: Base) -> Option<u64> {
    let radix = u64::from(base.radix());
    let mut value: u64 = 0;

    for c in digits.chars() {
        if c == '_' {
            continue;
        }
        let digit = u64::from(c.to_digit(base.radix())?);
        value = value.checked_mul(radix)?.checked_add(digit)?;
    }

    Some(value)
}

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
