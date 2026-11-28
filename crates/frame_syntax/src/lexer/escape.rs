//! Escapes, such as `\n` or `\u{1F600}`. Characters and texts share them.

use frame_diagnostics::{Diagnostic, Prefix, Primary, RuleCode};
use frame_source::{FileId, Span};

use super::usize_to_u32;
use Defect::{MalformedUnicode, NotScalarValue, Unknown};

const ESCAPE: RuleCode = RuleCode::new(Prefix::Lex, 16);

/// What's wrong with an invalid escape.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Defect {
    /// No escape starts with the character after `\`, as in `\q`.
    Unknown,
    /// A `\u` escape is not `\u{`, one to six hexadecimal digits, then `}`, as in `\u41`.
    MalformedUnicode,
    /// The value of a `\u{…}` escape isn't a Unicode scalar value, as in `\u{D800}`.
    NotScalarValue,
}

/// An invalid escape: what's wrong, and how far we read.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct EscapeError {
    defect: Defect,
    /// Bytes read, counting the `\`. The diagnostic underlines them and quotes them in its
    /// title.
    len: usize,
}

impl EscapeError {
    /// `text` starts with the escape, at byte `start` of the file.
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

    pub(super) fn len(&self) -> usize {
        self.len
    }
}

/// Reads the escape `text` starts with. Returns its character and its length in bytes.
///
/// An escape never takes a line ending: a `\` right before one, or at the end of `text`, is
/// an unknown escape on its own.
pub(super) fn read(text: &str) -> Result<(char, usize), EscapeError> {
    match text.chars().nth(1) {
        Some('n') => Ok(('\n', 2)),
        Some('r') => Ok(('\r', 2)),
        Some('t') => Ok(('\t', 2)),
        Some('0') => Ok(('\0', 2)),
        Some(c @ ('\\' | '\'' | '"' | '{' | '}')) => Ok((c, 2)),
        Some('u') => read_unicode(text),
        // Even a lone `\r`: it's a forbidden character (LEX-2), and the title mustn't quote it.
        None | Some('\n' | '\r') => Err(EscapeError {
            defect: Unknown,
            len: 1,
        }),
        Some(c) => Err(EscapeError {
            defect: Unknown,
            len: 1 + c.len_utf8(),
        }),
    }
}

fn read_unicode(text: &str) -> Result<(char, usize), EscapeError> {
    let Some(after_brace) = text.strip_prefix("\\u{") else {
        return Err(EscapeError {
            defect: MalformedUnicode,
            len: 2,
        });
    };

    // Hex digits are ASCII, so their count is also their length in bytes. `\u{` is three
    // bytes.
    let digit_count = after_brace
        .chars()
        .take_while(char::is_ascii_hexdigit)
        .count();

    if !after_brace[digit_count..].starts_with('}') {
        return Err(EscapeError {
            defect: MalformedUnicode,
            len: 3 + digit_count,
        });
    }

    // `\u{`, the digits, then `}`.
    let len = 3 + digit_count + 1;

    if !(1..=6).contains(&digit_count) {
        return Err(EscapeError {
            defect: MalformedUnicode,
            len,
        });
    }

    let value = u32::from_str_radix(&after_brace[..digit_count], 16)
        .expect("one to six hexadecimal digits fit in a u32");

    char::from_u32(value).map(|c| (c, len)).ok_or(EscapeError {
        defect: NotScalarValue,
        len,
    })
}

#[cfg(test)]
mod tests {
    use super::{Defect, EscapeError, read};

    use Defect::{MalformedUnicode, NotScalarValue, Unknown};

    type Read = Result<(char, usize), EscapeError>;

    fn invalid(defect: Defect, len: usize) -> Read {
        Err(EscapeError { defect, len })
    }

    /// Reads each text, and compares the result in full.
    fn assert_read(cases: &[(&str, Read)]) {
        for (text, expected) in cases {
            assert_eq!(&read(text), expected, "reading of {text:?}");
        }
    }

    #[test]
    fn simple_escapes_give_their_character() {
        assert_read(&[
            (r"\n", Ok(('\n', 2))),
            (r"\r", Ok(('\r', 2))),
            (r"\t", Ok(('\t', 2))),
            (r"\0", Ok(('\0', 2))),
            (r"\\", Ok(('\\', 2))),
            (r"\'", Ok(('\'', 2))),
            (r#"\""#, Ok(('"', 2))),
            (r"\{", Ok(('{', 2))),
            (r"\}", Ok(('}', 2))),
        ]);
    }

    #[test]
    fn unicode_escapes_give_their_value() {
        assert_read(&[
            (r"\u{41}", Ok(('A', 6))),
            (r"\u{e9}", Ok(('é', 6))),
            (r"\u{0}", Ok(('\0', 5))),
            (r"\u{1F600}", Ok(('😀', 9))),
            (r"\u{1f600}", Ok(('😀', 9))),
            (r"\u{10FFFF}", Ok(('\u{10FFFF}', 10))),
            // Six digits, leading zeros included.
            (r"\u{000041}", Ok(('A', 10))),
            // Just before and just after the surrogates.
            (r"\u{D7FF}", Ok(('\u{D7FF}', 8))),
            (r"\u{E000}", Ok(('\u{E000}', 8))),
        ]);
    }

    #[test]
    fn escape_reads_only_its_own_characters() {
        // The text goes on after the escape, so `read` must stop at its end.
        assert_read(&[
            (r"\nabc", Ok(('\n', 2))),
            (r"\\'", Ok(('\\', 2))),
            (r#"\u{41}"x"#, Ok(('A', 6))),
            (r"\u{41}}", Ok(('A', 6))),
        ]);
    }

    #[test]
    fn unknown_escape_is_rejected() {
        assert_read(&[
            (r"\q", invalid(Unknown, 2)),
            (r"\U", invalid(Unknown, 2)),
            (r"\x41", invalid(Unknown, 2)),
            // The span takes the whole character after `\`.
            (r"\é", invalid(Unknown, 3)),
            // Only `\` was read.
            (r"\", invalid(Unknown, 1)),
            // An escape never takes a line ending.
            ("\\\nx", invalid(Unknown, 1)),
            ("\\\r\nx", invalid(Unknown, 1)),
        ]);
    }

    #[test]
    fn malformed_unicode_escape_is_rejected() {
        assert_read(&[
            // No `{`: only `\u` was read.
            (r"\u41", invalid(MalformedUnicode, 2)),
            (r"\u", invalid(MalformedUnicode, 2)),
            // No `}` after the digits: `\u{` and the digits were read.
            (r"\u{4G}", invalid(MalformedUnicode, 4)),
            (r"\u{41", invalid(MalformedUnicode, 5)),
            (r"\u{41'", invalid(MalformedUnicode, 5)),
            // `from_str_radix` would accept the sign.
            (r"\u{+41}", invalid(MalformedUnicode, 3)),
            // `}` after the digits: the whole escape was read.
            (r"\u{}", invalid(MalformedUnicode, 4)),
            (r"\u{1234567}", invalid(MalformedUnicode, 11)),
            // Seven digits are rejected even when the value is valid.
            (r"\u{0000041}", invalid(MalformedUnicode, 11)),
        ]);
    }

    #[test]
    fn unicode_escape_outside_scalar_values_is_rejected() {
        assert_read(&[
            (r"\u{D800}", invalid(NotScalarValue, 8)),
            (r"\u{DFFF}", invalid(NotScalarValue, 8)),
            (r"\u{110000}", invalid(NotScalarValue, 10)),
            (r"\u{FFFFFF}", invalid(NotScalarValue, 10)),
        ]);
    }
}
