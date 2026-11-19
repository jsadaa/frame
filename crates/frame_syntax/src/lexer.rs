//! The lexer, which splits a file's text into tokens.

use std::fmt::Write;
use std::str::Chars;

use frame_diagnostics::{Diagnostic, Prefix, Primary, RuleCode};
use frame_source::{FileId, SourceFile, SourceMap, Span};

const UNEXPECTED_CHARACTER: RuleCode = RuleCode::new(Prefix::Lex, 24);

fn unexpected_character(c: char, span: Span) -> Diagnostic {
    Diagnostic::new(
        UNEXPECTED_CHARACTER,
        format!("unexpected character `{c}`"),
        Primary::span(span),
    )
}

/// A token: its kind, and the span of text it covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    kind: TokenKind,
    span: Span,
}

impl Token {
    fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }

    /// Returns the token's kind.
    pub fn kind(&self) -> &TokenKind {
        &self.kind
    }

    /// Returns the span of text the token covers.
    pub fn span(&self) -> Span {
        self.span
    }
}

/// The kind of a token.
///
/// Operators and punctuation are named by their shape, since only the parser knows what they
/// mean: `|` is `Pipe`, not `Or`.
// Not `Copy`: the kinds of literals will carry their value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `*`
    Star,
    /// `/`
    Slash,
    /// `%`
    Percent,
    /// `==`
    EqEq,
    /// `!=`
    BangEq,
    /// `<`
    Lt,
    /// `<=`
    LtEq,
    /// `>`
    Gt,
    /// `>=`
    GtEq,
    /// `&`
    Amp,
    /// `|`
    Pipe,
    /// `^`
    Caret,
    /// `~`
    Tilde,
    /// `<<`
    Shl,
    /// `>>`
    Shr,
    /// `=`
    Eq,
    /// `+=`
    PlusEq,
    /// `-=`
    MinusEq,
    /// `*=`
    StarEq,
    /// `/=`
    SlashEq,
    /// `%=`
    PercentEq,
    /// `&=`
    AmpEq,
    /// `|=`
    PipeEq,
    /// `^=`
    CaretEq,
    /// `<<=`
    ShlEq,
    /// `>>=`
    ShrEq,
    /// `..`
    DotDot,
    /// `..=`
    DotDotEq,
    /// `.`
    Dot,
    /// `?.`
    QuestionDot,
    /// `->`
    Arrow,
    /// `=>`
    FatArrow,
    /// `(`
    OpenParen,
    /// `)`
    CloseParen,
    /// `[`
    OpenBracket,
    /// `]`
    CloseBracket,
    /// `{`
    OpenBrace,
    /// `}`
    CloseBrace,
    /// `,`
    Comma,
    /// `:`
    Colon,
    /// `?`
    Question,
    /// `!`
    Bang,
    /// A line ending: `\n` or `\r\n`.
    Newline,
    /// The end of the text. Its span is empty.
    Eof,
}

// Longest first: `find` stops at the first match, which must be the longest (LEX-7).
const OPERATORS: [(&str, TokenKind); 44] = [
    ("<<=", TokenKind::ShlEq),
    (">>=", TokenKind::ShrEq),
    ("..=", TokenKind::DotDotEq),
    ("==", TokenKind::EqEq),
    ("!=", TokenKind::BangEq),
    ("<=", TokenKind::LtEq),
    (">=", TokenKind::GtEq),
    ("<<", TokenKind::Shl),
    (">>", TokenKind::Shr),
    ("+=", TokenKind::PlusEq),
    ("-=", TokenKind::MinusEq),
    ("*=", TokenKind::StarEq),
    ("/=", TokenKind::SlashEq),
    ("%=", TokenKind::PercentEq),
    ("&=", TokenKind::AmpEq),
    ("|=", TokenKind::PipeEq),
    ("^=", TokenKind::CaretEq),
    ("..", TokenKind::DotDot),
    ("?.", TokenKind::QuestionDot),
    ("->", TokenKind::Arrow),
    ("=>", TokenKind::FatArrow),
    ("+", TokenKind::Plus),
    ("-", TokenKind::Minus),
    ("*", TokenKind::Star),
    ("/", TokenKind::Slash),
    ("%", TokenKind::Percent),
    ("<", TokenKind::Lt),
    (">", TokenKind::Gt),
    ("&", TokenKind::Amp),
    ("|", TokenKind::Pipe),
    ("^", TokenKind::Caret),
    ("~", TokenKind::Tilde),
    ("=", TokenKind::Eq),
    (".", TokenKind::Dot),
    ("(", TokenKind::OpenParen),
    (")", TokenKind::CloseParen),
    ("[", TokenKind::OpenBracket),
    ("]", TokenKind::CloseBracket),
    ("{", TokenKind::OpenBrace),
    ("}", TokenKind::CloseBrace),
    (",", TokenKind::Comma),
    (":", TokenKind::Colon),
    ("?", TokenKind::Question),
    ("!", TokenKind::Bang),
];

/// Reads a text one character at a time.
///
/// It moves through a `Chars`, so a position can never fall inside a multibyte character.
struct Cursor<'a> {
    chars: Chars<'a>,
    text_len: usize,
}

impl<'a> Cursor<'a> {
    fn first(&self) -> Option<char> {
        self.chars.clone().next()
    }

    fn second(&self) -> Option<char> {
        // Two `next` calls optimize better than `nth(1)`, as in rustc_lexer's cursor.
        let mut iter = self.chars.clone();
        iter.next();
        iter.next()
    }

    fn advance(&mut self) -> Option<char> {
        self.chars.next()
    }

    fn as_str(&self) -> &'a str {
        self.chars.as_str()
    }

    /// Returns the byte offset of the next character.
    fn position(&self) -> usize {
        self.text_len - self.chars.as_str().len()
    }
}

struct Lexer<'a> {
    file: FileId,
    cursor: Cursor<'a>,
    diagnostics: Vec<Diagnostic>,
    tokens: Vec<Token>,
}

impl<'a> Lexer<'a> {
    fn new(text: &'a str, file: FileId) -> Self {
        let cursor = Cursor {
            chars: text.chars(),
            text_len: text.len(),
        };

        Self {
            file,
            cursor,
            diagnostics: Vec::new(),
            tokens: Vec::new(),
        }
    }

    fn run(mut self) -> Lexed {
        while let Some(c) = self.cursor.first() {
            let before = self.cursor.position();

            match c {
                ' ' | '\t' => {
                    self.cursor.advance();
                }
                '\n' | '\r' => {
                    if let Some(token) = self.read_newline() {
                        self.tokens.push(token);
                    } else {
                        // TODO(J02): refuse a lone `\r` as a forbidden character (LEX-2).
                        self.skip_unexpected(c);
                    }
                }
                _ => {
                    if let Some(token) = self.read_operator() {
                        self.tokens.push(token);
                    } else {
                        self.skip_unexpected(c);
                    }
                }
            }

            // An arm that does not move the cursor would loop forever, and hang the tests.
            debug_assert!(
                self.cursor.position() > before,
                "the lexer did not move at byte {before}"
            );
        }

        self.tokens.push(Token::new(
            TokenKind::Eof,
            self.span_from(self.cursor.position()),
        ));

        Lexed {
            tokens: self.tokens,
            diagnostics: self.diagnostics,
        }
    }

    /// Reads a line ending, `\n` or `\r\n`, if one starts here.
    fn read_newline(&mut self) -> Option<Token> {
        let start = self.cursor.position();

        match self.cursor.first() {
            Some('\n') => {
                self.cursor.advance();
                Some(Token::new(TokenKind::Newline, self.span_from(start)))
            }
            Some('\r') if self.cursor.second() == Some('\n') => {
                self.cursor.advance();
                self.cursor.advance();
                Some(Token::new(TokenKind::Newline, self.span_from(start)))
            }
            _ => None,
        }
    }

    /// Reads the longest operator or punctuation token that starts here.
    fn read_operator(&mut self) -> Option<Token> {
        OPERATORS
            .iter()
            .find(|(text, _)| self.cursor.as_str().starts_with(text))
            .map(|(text, kind)| {
                let start = self.cursor.position();

                for _ in text.chars() {
                    self.cursor.advance();
                }

                Token::new(kind.clone(), self.span_from(start))
            })
    }

    /// Skips `c`, which starts no token, and reports it.
    fn skip_unexpected(&mut self, c: char) {
        let start = self.cursor.position();
        self.cursor.advance();
        self.diagnostics
            .push(unexpected_character(c, self.span_from(start)));
    }

    /// Returns the span from `start` to the cursor.
    fn span_from(&self, start: usize) -> Span {
        Span::new(
            self.file,
            usize_to_u32(start),
            usize_to_u32(self.cursor.position()),
        )
    }
}

/// The tokens of a file, and the diagnostics found while reading it.
#[derive(Debug)]
pub struct Lexed {
    tokens: Vec<Token>,
    diagnostics: Vec<Diagnostic>,
}

impl Lexed {
    /// Returns the tokens in text order. The last one marks the end of the file.
    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    /// Returns the diagnostics in text order.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

/// Splits `file`'s text into tokens.
///
/// - Spaces and tabs only separate tokens.
/// - Each line ending, `\n` or `\r\n`, gives a newline token.
/// - Operators and punctuation are read as the longest token that matches: `<<=` is one
///   token, not `<<` then `=`.
/// - Any other character gets a diagnostic, and reading goes on after it.
///
/// The last token marks the end of the file, with an empty span.
///
/// # Panics
///
/// Panics if `file` is not in `source_map`.
pub fn tokenize(source_map: &SourceMap, file: FileId) -> Lexed {
    let text = source_map.file(file).text();
    Lexer::new(text, file).run()
}

/// Writes tokens as text, one per line.
///
/// A line gives the token's start as `line:column`, then its kind and its text, both written
/// by `Debug`:
///
/// ```text
/// 1:1 Plus "+"
/// 1:3 ShlEq "<<="
/// 1:6 Newline "\n"
/// 2:1 Eof ""
/// ```
///
/// # Panics
///
/// Panics if a token's span is not within `file`'s text.
pub fn dump_tokens(tokens: &[Token], file: &SourceFile) -> String {
    let mut dump = String::new();

    for token in tokens {
        let span = token.span();
        let line_col = file.line_col(span.start());
        let token_text = &file.text()[span.start() as usize..span.end() as usize];
        writeln!(dump, "{line_col} {:?} {token_text:?}", token.kind())
            .expect("writing to a String should not fail");
    }

    dump
}

fn usize_to_u32(value: usize) -> u32 {
    u32::try_from(value).expect("SourceMap::add() checked that offsets fit in u32")
}

#[cfg(test)]
mod tests {
    use super::{dump_tokens, tokenize};
    use frame_diagnostics::{Diagnostic, Prefix, Primary, RuleCode};
    use frame_source::{SourceMap, Span};
    use std::ops::Range;

    const UNEXPECTED_CHARACTER: RuleCode = RuleCode::new(Prefix::Lex, 24);

    /// Tokenizes `text`, alone in a source map, and compares the result in full.
    ///
    /// `expected_tokens` holds one line of `dump_tokens` per token, without its `\n`.
    /// `unexpected_characters` holds each unexpected character and its byte range, in text
    /// order.
    fn assert_lexed(
        text: &str,
        expected_tokens: &[&str],
        unexpected_characters: &[(char, Range<u32>)],
    ) {
        let mut source_map = SourceMap::new();
        let file = source_map.add(String::from("test.fm"), text.to_string());
        let lexed = tokenize(&source_map, file);

        let mut expected_dump = String::new();
        for line in expected_tokens {
            expected_dump.push_str(line);
            expected_dump.push('\n');
        }
        let expected_diagnostics: Vec<Diagnostic> = unexpected_characters
            .iter()
            .map(|(c, range)| {
                Diagnostic::new(
                    UNEXPECTED_CHARACTER,
                    format!("unexpected character `{c}`"),
                    Primary::span(Span::new(file, range.start, range.end)),
                )
            })
            .collect();

        assert_eq!(
            dump_tokens(lexed.tokens(), source_map.file(file)),
            expected_dump,
            "tokens of {text:?}"
        );
        assert_eq!(
            lexed.diagnostics(),
            expected_diagnostics,
            "diagnostics of {text:?}"
        );
    }

    #[test]
    fn empty_text_gives_only_the_end_of_file() {
        assert_lexed("", &[r#"1:1 Eof """#], &[]);
    }

    #[test]
    fn spaces_and_tabs_give_no_token() {
        assert_lexed(" \t ", &[r#"1:4 Eof """#], &[]);
    }

    #[test]
    fn each_operator_is_one_token() {
        // Written from the specification's table, in its order, not from the lexer's.
        let operators: [(&str, &str); 44] = [
            ("+", "Plus"),
            ("-", "Minus"),
            ("*", "Star"),
            ("/", "Slash"),
            ("%", "Percent"),
            ("==", "EqEq"),
            ("!=", "BangEq"),
            ("<", "Lt"),
            ("<=", "LtEq"),
            (">", "Gt"),
            (">=", "GtEq"),
            ("&", "Amp"),
            ("|", "Pipe"),
            ("^", "Caret"),
            ("~", "Tilde"),
            ("<<", "Shl"),
            (">>", "Shr"),
            ("=", "Eq"),
            ("+=", "PlusEq"),
            ("-=", "MinusEq"),
            ("*=", "StarEq"),
            ("/=", "SlashEq"),
            ("%=", "PercentEq"),
            ("&=", "AmpEq"),
            ("|=", "PipeEq"),
            ("^=", "CaretEq"),
            ("<<=", "ShlEq"),
            (">>=", "ShrEq"),
            ("..", "DotDot"),
            ("..=", "DotDotEq"),
            (".", "Dot"),
            ("?.", "QuestionDot"),
            ("->", "Arrow"),
            ("=>", "FatArrow"),
            ("(", "OpenParen"),
            (")", "CloseParen"),
            ("[", "OpenBracket"),
            ("]", "CloseBracket"),
            ("{", "OpenBrace"),
            ("}", "CloseBrace"),
            (",", "Comma"),
            (":", "Colon"),
            ("?", "Question"),
            ("!", "Bang"),
        ];

        for (text, kind) in operators {
            let token = format!("1:1 {kind} {text:?}");
            let end = format!(r#"1:{} Eof """#, text.len() + 1);
            assert_lexed(text, &[&token, &end], &[]);
        }
    }

    #[test]
    fn longest_token_wins() {
        let cases: [(&str, &[&str]); 9] = [
            ("<<=", &[r#"1:1 ShlEq "<<=""#, r#"1:4 Eof """#]),
            (
                "<< =",
                &[r#"1:1 Shl "<<""#, r#"1:4 Eq "=""#, r#"1:5 Eof """#],
            ),
            (
                "...",
                &[r#"1:1 DotDot "..""#, r#"1:3 Dot ".""#, r#"1:4 Eof """#],
            ),
            (
                "?..",
                &[r#"1:1 QuestionDot "?.""#, r#"1:3 Dot ".""#, r#"1:4 Eof """#],
            ),
            (
                "=>=",
                &[r#"1:1 FatArrow "=>""#, r#"1:3 Eq "=""#, r#"1:4 Eof """#],
            ),
            (
                "!==",
                &[r#"1:1 BangEq "!=""#, r#"1:3 Eq "=""#, r#"1:4 Eof """#],
            ),
            (
                "->>",
                &[r#"1:1 Arrow "->""#, r#"1:3 Gt ">""#, r#"1:4 Eof """#],
            ),
            (
                ">>>",
                &[r#"1:1 Shr ">>""#, r#"1:3 Gt ">""#, r#"1:4 Eof """#],
            ),
            (
                "++",
                &[r#"1:1 Plus "+""#, r#"1:2 Plus "+""#, r#"1:3 Eof """#],
            ),
        ];

        for (text, expected_tokens) in cases {
            assert_lexed(text, expected_tokens, &[]);
        }
    }

    #[test]
    fn each_line_ending_gives_a_newline_token() {
        let cases: [(&str, &[&str]); 3] = [
            (
                "+\n-",
                &[
                    r#"1:1 Plus "+""#,
                    r#"1:2 Newline "\n""#,
                    r#"2:1 Minus "-""#,
                    r#"2:2 Eof """#,
                ],
            ),
            (
                "+\r\n-",
                &[
                    r#"1:1 Plus "+""#,
                    r#"1:2 Newline "\r\n""#,
                    r#"2:1 Minus "-""#,
                    r#"2:2 Eof """#,
                ],
            ),
            (
                "+\n",
                &[r#"1:1 Plus "+""#, r#"1:2 Newline "\n""#, r#"2:1 Eof """#],
            ),
        ];

        for (text, expected_tokens) in cases {
            assert_lexed(text, expected_tokens, &[]);
        }
    }

    #[test]
    fn unexpected_character_is_reported_and_skipped() {
        assert_lexed("@", &[r#"1:2 Eof """#], &[('@', 0..1)]);
    }

    #[test]
    fn unexpected_character_covers_all_its_bytes() {
        assert_lexed(
            "+é-",
            &[r#"1:1 Plus "+""#, r#"1:3 Minus "-""#, r#"1:4 Eof """#],
            &[('é', 1..3)],
        );
    }

    #[test]
    fn every_unexpected_character_is_reported_in_text_order() {
        assert_lexed("@ $", &[r#"1:4 Eof """#], &[('@', 0..1), ('$', 2..3)]);
    }

    #[test]
    fn lone_carriage_return_is_an_unexpected_character() {
        assert_lexed(
            "+\r-",
            &[r#"1:1 Plus "+""#, r#"1:3 Minus "-""#, r#"1:4 Eof """#],
            &[('\r', 1..2)],
        );
    }

    #[test]
    fn no_break_space_is_an_unexpected_character() {
        // Only the space and the tab separate tokens (LEX-4).
        assert_lexed(
            "+\u{A0}-",
            &[r#"1:1 Plus "+""#, r#"1:3 Minus "-""#, r#"1:4 Eof """#],
            &[('\u{A0}', 1..3)],
        );
    }
}
