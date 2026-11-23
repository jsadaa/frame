use std::ops::Range;

use crate::{
    TokenKind,
    lexer::number::Defect::{
        InvalidDigit, LeadingZero, MisplacedUnderscore, MissingDigits, UppercasePrefix,
    },
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

    let (base, upercase_prefix, number_offset) = match (bytes.first(), bytes.get(1)) {
        (Some(b'0'), Some(&c @ (b'b' | b'B'))) => (Base::Binary, c.is_ascii_uppercase(), 2),
        (Some(b'0'), Some(&c @ (b'o' | b'O'))) => (Base::Octal, c.is_ascii_uppercase(), 2),
        (Some(b'0'), Some(&c @ (b'x' | b'X'))) => (Base::Hexadecimal, c.is_ascii_uppercase(), 2),
        _ => (Base::Decimal, false, 0),
    };

    let is_float = text.contains('.')
        || (base == Base::Decimal && (0..bytes.len()).any(|i| is_exponent_at(bytes, i)));

    if upercase_prefix {
        return Err(NumberError {
            defect: UppercasePrefix,
            range: 0..2,
            is_float,
        });
    }

    if base == Base::Decimal && bytes.first() == Some(&b'0') {
        return Err(NumberError {
            defect: LeadingZero,
            range: 0..1,
            is_float,
        });
    }

    match bytes.get(2) {
        None => {
            return Err(NumberError {
                defect: MissingDigits,
                range: (0..bytes.len()),
                is_float,
            });
        }
        Some(c) if !char::from(*c).is_digit(base.radix()) => {
            return Err(NumberError {
                defect: InvalidDigit(base),
                range: 2..3,
                is_float,
            });
        }
        _ => {}
    }

    todo!()
}

fn scan_digits(
    bytes: &[u8],
    start: usize,
    base: Base,
    is_float: bool,
) -> Result<usize, NumberError> {
    let in_base = |c: u8| char::from(c).is_digit(base.radix());

    for i in start..bytes.len() {
        let c = bytes[i];

        match c {
            b'0'..=b'9' | b'a'..=b'f' | b'A'..=b'F' => {
                if !in_base(c) {
                    return Err(NumberError {
                        defect: InvalidDigit(base),
                        range: i..(i + 1),
                        is_float,
                    });
                }
            }
            b'_' => {
                if i == start
                    || i + 1 == bytes.len()
                    || !in_base(bytes[i - 1])
                    || !in_base(bytes[i + 1])
                {
                    return Err(NumberError {
                        defect: MisplacedUnderscore,
                        range: i..(i + 1),
                        is_float,
                    });
                }
            }
            _ => return Ok(i),
        }
    }

    Ok(bytes.len())
}

fn is_exponent_at(bytes: &[u8], i: usize) -> bool {
    if i >= bytes.len() {
        return false;
    }

    let c = bytes[i];

    if c != b'e' && c != b'E' {
        return false;
    }

    match bytes.get(i + 1) {
        Some(b'0'..=b'9') => true,
        Some(b'+' | b'-') => bytes.get(i + 2).is_some_and(|&c| c.is_ascii_digit()),
        _ => false,
    }
}
