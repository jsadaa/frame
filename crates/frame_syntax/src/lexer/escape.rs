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

/// Reads the escape at the start of `text`, which starts with `\`. Returns its character
/// and how many bytes it takes.
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
        // The text may end right after `\`.
        None => Err(EscapeError {
            defect: Unknown,
            len: 1,
        }),
    }
}

/// Reads a `\u{…}` escape at the start of `text`.
fn read_unicode(text: &str) -> Result<(char, usize), EscapeError> {
    let Some(after_brace) = text.strip_prefix("\\u{") else {
        return Err(EscapeError {
            defect: MalformedUnicode,
            len: 2,
        });
    };

    // Hexadecimal digits are ASCII, so their count is also their length in bytes. `\u{` takes
    // three.
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

    if !(1..=6).contains(&digit_count) {
        return Err(EscapeError {
            defect: MalformedUnicode,
            len: 3 + digit_count + 1,
        });
    }

    let value = u32::from_str_radix(&after_brace[..digit_count], 16)
        .expect("one to six hexadecimal digits fit in a u32");

    match char::from_u32(value) {
        Some(c) => Ok((c, 3 + digit_count + 1)),
        None => Err(EscapeError {
            defect: NotScalarValue,
            len: 3 + digit_count + 1,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{Defect, EscapeError, read};

    use Defect::{MalformedUnicode, NotScalarValue, Unknown};

    type Read = Result<(char, usize), EscapeError>;

    fn refused(defect: Defect, len: usize) -> Read {
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
        // A text goes on after its escapes, so the reader stops at the escape's end.
        assert_read(&[
            (r"\nabc", Ok(('\n', 2))),
            (r"\\'", Ok(('\\', 2))),
            (r#"\u{41}"x"#, Ok(('A', 6))),
            (r"\u{41}}", Ok(('A', 6))),
        ]);
    }

    #[test]
    fn unknown_escape_is_refused() {
        assert_read(&[
            (r"\q", refused(Unknown, 2)),
            (r"\U", refused(Unknown, 2)),
            (r"\x41", refused(Unknown, 2)),
            // The span takes the whole character after `\`.
            (r"\é", refused(Unknown, 3)),
            // Only `\` was read.
            (r"\", refused(Unknown, 1)),
        ]);
    }

    #[test]
    fn malformed_unicode_escape_is_refused() {
        assert_read(&[
            // No `{`: only `\u` was read.
            (r"\u41", refused(MalformedUnicode, 2)),
            (r"\u", refused(MalformedUnicode, 2)),
            // No `}` after the digits: `\u{` and the digits were read.
            (r"\u{4G}", refused(MalformedUnicode, 4)),
            (r"\u{41", refused(MalformedUnicode, 5)),
            (r"\u{41'", refused(MalformedUnicode, 5)),
            // `from_str_radix` would accept the sign.
            (r"\u{+41}", refused(MalformedUnicode, 3)),
            // `}` after the digits: the whole escape was read.
            (r"\u{}", refused(MalformedUnicode, 4)),
            (r"\u{1234567}", refused(MalformedUnicode, 11)),
            // Seven digits are refused even when the value is valid.
            (r"\u{0000041}", refused(MalformedUnicode, 11)),
        ]);
    }

    #[test]
    fn unicode_escape_outside_scalar_values_is_refused() {
        assert_read(&[
            (r"\u{D800}", refused(NotScalarValue, 8)),
            (r"\u{DFFF}", refused(NotScalarValue, 8)),
            (r"\u{110000}", refused(NotScalarValue, 10)),
            (r"\u{FFFFFF}", refused(NotScalarValue, 10)),
        ]);
    }
}
