//! Escapes, such as `\n` or `\u{1F600}`. They are the same in characters and in texts.

use frame_diagnostics::{Diagnostic, Prefix, Primary, RuleCode};
use frame_source::{FileId, Span};

use super::usize_to_u32;
use Defect::{MalformedUnicode, NotScalarValue, Unknown};

const ESCAPE: RuleCode = RuleCode::new(Prefix::Lex, 16);

/// What is wrong with a refused escape.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Defect {
    /// The character after `\` starts no escape, as in `\q`.
    Unknown,
    /// A `\u` escape is not `\u{`, one to six hexadecimal digits, then `}`, as in `\u41`.
    MalformedUnicode,
    /// The digits of a `\u{…}` escape do not give a Unicode scalar value, as in `\u{D800}`.
    NotScalarValue,
}

/// A refused escape: its defect, and how far it was read.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct EscapeError {
    defect: Defect,
    /// The bytes read, counted from the `\`. The diagnostic covers them, and its title quotes
    /// them.
    len: usize,
}

impl EscapeError {
    /// Builds the diagnostic of this escape, for the text `text` that starts with it at byte
    /// `start`.
    pub(super) fn to_diagnostic(&self, text: &str, file: FileId, start: usize) -> Diagnostic {
        let span = Span::new(file, usize_to_u32(start), usize_to_u32(start + self.len));
        let found = &text[..self.len];

        match self.defect {
            Unknown => Diagnostic::new(
                ESCAPE,
                format!("unknown escape `{found}`"),
                Primary::span(span),
            )
            .with_help(r#"the escapes are `\n` `\r` `\t` `\0` `\\` `\'` `\"` `\{` `\}` `\u{…}`"#),
            MalformedUnicode => {
                Diagnostic::new(ESCAPE, "malformed Unicode escape", Primary::span(span))
                    .with_help("write `\\u{` with one to six hexadecimal digits, then `}`")
            }
            NotScalarValue => Diagnostic::new(
                ESCAPE,
                format!("`{found}` is not a Unicode scalar value"),
                Primary::span(span),
            ),
        }
    }
}

pub(super) fn read(text: &str) -> Result<(char, usize), EscapeError> {
    match text.chars().nth(1) {
        Some('n') => Ok(('\n', 2)),
        Some('r') => Ok(('\r', 2)),
        Some('t') => Ok(('\t', 2)),
        Some('0') => Ok(('\0', 2)),
        Some(c @ ('\\' | '\'' | '"' | '{' | '}')) => Ok((c, 2)),
        Some('u') => read_unicode(text),
        Some(c) => Err(EscapeError {
            defect: Unknown,
            len: 1 + c.len_utf8(),
        }),
        None => Err(EscapeError {
            defect: Unknown,
            len: 1,
        }),
    }
}

fn read_unicode(text: &str) -> Result<(char, usize), EscapeError> {
    let Some(stripped) = text.strip_prefix("\\u{") else {
        return Err(EscapeError {
            defect: MalformedUnicode,
            len: 2,
        });
    };

    let n = stripped.chars().take_while(char::is_ascii_hexdigit).count();

    if !stripped[n..].starts_with('}') {
        return Err(EscapeError {
            defect: MalformedUnicode,
            len: 3 + n,
        });
    }

    if !(1..=6).contains(&n) {
        return Err(EscapeError {
            defect: MalformedUnicode,
            len: 3 + n + 1,
        });
    }

    let value = u32::from_str_radix(&stripped[..n], 16)
        .expect("one to six hexadecimal digits fit in a u32");

    match char::from_u32(value) {
        Some(c) => Ok((c, 3 + n + 1)),
        None => Err(EscapeError {
            defect: NotScalarValue,
            len: 3 + n + 1,
        }),
    }
}
