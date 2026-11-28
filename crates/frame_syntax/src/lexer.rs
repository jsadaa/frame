//! The lexer, which splits a file's text into tokens.

mod escape;
mod number;

use std::fmt::Write;
use std::str::Chars;

use frame_diagnostics::{Diagnostic, Prefix, Primary, RuleCode};
use frame_source::{FileId, SourceFile, SourceMap, Span};

const UNEXPECTED_CHARACTER: RuleCode = RuleCode::new(Prefix::Lex, 24);
const DOC_COMMENT_NOT_ALONE_ON_ITS_LINE: RuleCode = RuleCode::new(Prefix::Lex, 6);
const RESERVED_WORD: RuleCode = RuleCode::new(Prefix::Lex, 11);
const CHARACTER_LITERAL: RuleCode = RuleCode::new(Prefix::Lex, 15);
const RAW_TEXT: RuleCode = RuleCode::new(Prefix::Lex, 19);
const TEXT_LITERAL: RuleCode = RuleCode::new(Prefix::Lex, 17);
const INTERPOLATION: RuleCode = RuleCode::new(Prefix::Lex, 20);

// Kept for future versions. None has a use yet, so each one is a name, and the lexer can
// refuse it without knowing what the word declares.
const RESERVED_WORDS: [&str; 8] = [
    "realize", "flow", "under", "unsafe", "extern", "comptime", "yield", "export",
];

fn unexpected_character(c: char, span: Span) -> Diagnostic {
    Diagnostic::new(
        UNEXPECTED_CHARACTER,
        format!("unexpected character `{c}`"),
        Primary::span(span),
    )
}

fn doc_comment_not_alone_on_its_line(span: Span) -> Diagnostic {
    Diagnostic::new(
        DOC_COMMENT_NOT_ALONE_ON_ITS_LINE,
        "doc comment is not alone on its line",
        Primary::span(span),
    )
    .with_help("for an ordinary comment, write `//`")
}

fn reserved_word(word: &str, span: Span) -> Diagnostic {
    Diagnostic::new(
        RESERVED_WORD,
        format!("`{word}` is reserved"),
        Primary::span(span),
    )
    .with_help("this word is reserved for a future version of Frame; choose another name")
}

fn unterminated_character_literal(span: Span) -> Diagnostic {
    Diagnostic::new(
        CHARACTER_LITERAL,
        "unterminated character literal",
        Primary::span(span),
    )
}

fn empty_character_literal(span: Span) -> Diagnostic {
    Diagnostic::new(
        CHARACTER_LITERAL,
        "empty character literal",
        Primary::span(span),
    )
}

fn character_literal_with_more_than_one_character(content: &str, span: Span) -> Diagnostic {
    let text = escape_for_text(content);

    Diagnostic::new(
        CHARACTER_LITERAL,
        "character literal with more than one character",
        Primary::span(span),
    )
    .with_help(format!("for a text, write `\"{text}\"`"))
}

fn unterminated_raw_text(span: Span) -> Diagnostic {
    Diagnostic::new(RAW_TEXT, "unterminated raw text", Primary::span(span))
        .with_help("a raw text fits on one line and contains no `\"`")
}

fn unterminated_text(span: Span) -> Diagnostic {
    Diagnostic::new(TEXT_LITERAL, "unterminated text", Primary::span(span))
        .with_help(r#"a text fits on one line; for several lines, write `"""`"#)
}

fn unmatched_brace_in_text(span: Span) -> Diagnostic {
    Diagnostic::new(TEXT_LITERAL, "unmatched `}` in a text", Primary::span(span))
        .with_help(r"for a literal brace, write `\}`")
}

fn empty_interpolation(span: Span) -> Diagnostic {
    Diagnostic::new(INTERPOLATION, "empty interpolation", Primary::span(span))
}

fn unterminated_interpolation(span: Span) -> Diagnostic {
    Diagnostic::new(
        INTERPOLATION,
        "unterminated interpolation",
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
// Not `Copy`: some kinds carry a value, such as a doc comment's text.
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
    /// A doc comment, `///`, alone on its line.
    ///
    /// It holds the comment's text, after `///` and one optional space.
    DocComment(String),
    /// A keyword, such as `let`.
    Keyword(Keyword),
    /// An identifier, such as `total`. Its name is the token's text.
    Identifier,
    /// An integer literal, such as `0xFF`. It holds the value, or `None` if the value does not
    /// fit in a `u64`.
    Int(Option<u64>),
    /// A float literal, such as `2.5e-3`. It holds the text without `_`, ready for `parse`.
    Float(String),
    /// A character literal, such as `'a'` or `'\n'`. It holds the value, or U+FFFD if the
    /// literal is refused.
    Char(char),
    /// A raw text, such as `r"C:\Users"`. It holds the characters between the quotes, as
    /// written; without the closing quote, those up to the end of the line.
    RawText(String),
    /// The `"` that opens a text.
    ///
    /// A text gives this token, then its pieces and interpolations, then `TextEnd`.
    TextStart,
    /// Characters of a text, between its quotes and its interpolations. It holds them with
    /// their escapes read: `\t` gives a tab, and a refused escape gives U+FFFD. It is never
    /// empty.
    TextPiece(String),
    /// The `"` that closes a text. Without it, the text ends at the end of the line, and the
    /// span is empty.
    TextEnd,
    /// The `{` that opens an interpolation in a text.
    InterpolationStart,
    /// The `}` that closes an interpolation. Without it, the interpolation ends at the end of
    /// the line, and the span is empty.
    InterpolationEnd,
    /// The format of an interpolation, such as `:>8` in `"{cents:>8}"`. It covers the `:`, and
    /// holds what follows it as written, up to the `}`.
    Format(String),
    /// `_` alone, the wildcard of patterns. `_x` is an identifier.
    Underscore,
    /// A line ending: `\n` or `\r\n`.
    Newline,
    /// The end of the text. Its span is empty.
    Eof,
}

/// What waits for its closing token: a delimiter of code, or an interpolation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Open {
    Delimiter(Delimiter),
    Interpolation(Interpolation),
}

/// A delimiter of code. As a `usize`, it indexes `Lexer::open_counts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Delimiter {
    Paren,
    Bracket,
    Brace,
}

/// An interpolation not closed yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Interpolation {
    /// Where its text starts, at the `"`.
    text_start: usize,
    /// Where it starts, at the `{`.
    brace_start: usize,
    /// The counts of the delimiters open outside it, put back when it closes.
    outer_counts: [usize; 3],
}

/// A keyword.
///
/// Case counts: `self` and `Self` are two keywords, and `Fn` is an identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyword {
    /// `fn`
    Fn,
    /// `type`
    Type,
    /// `enum`
    Enum,
    /// `alias`
    Alias,
    /// `interface`
    Interface,
    /// `impl`
    Impl,
    /// `effect`
    Effect,
    /// `const`
    Const,
    /// `test`
    Test,
    /// `import`
    Import,
    /// `pub`
    Pub,
    /// `let`
    Let,
    /// `var`
    Var,
    /// `mut`
    Mut,
    /// `if`
    If,
    /// `else`
    Else,
    /// `match`
    Match,
    /// `when`
    When,
    /// `for`
    For,
    /// `in`
    In,
    /// `while`
    While,
    /// `loop`
    Loop,
    /// `break`
    Break,
    /// `continue`
    Continue,
    /// `return`
    Return,
    /// `fails`
    Fails,
    /// `fail`
    Fail,
    /// `try`
    Try,
    /// `attempt`
    Attempt,
    /// `requires`
    Requires,
    /// `assert`
    Assert,
    /// `uses`
    Uses,
    /// `provide`
    Provide,
    /// `with`
    With,
    /// `where`
    Where,
    /// `any`
    Any,
    // A variant cannot be named `Self`, a Rust keyword: the two take rustc's names.
    /// `Self`
    SelfUpper,
    /// `self`
    SelfLower,
    /// `and`
    And,
    /// `or`
    Or,
    /// `not`
    Not,
    /// `is`
    Is,
    /// `as`
    As,
    /// `true`
    True,
    /// `false`
    False,
    /// `none`
    None,
    /// `it`
    It,
}

impl Keyword {
    /// Returns the keyword written `text`, if there is one.
    fn from_text(text: &str) -> Option<Self> {
        match text {
            "fn" => Some(Keyword::Fn),
            "type" => Some(Keyword::Type),
            "enum" => Some(Keyword::Enum),
            "alias" => Some(Keyword::Alias),
            "interface" => Some(Keyword::Interface),
            "impl" => Some(Keyword::Impl),
            "effect" => Some(Keyword::Effect),
            "const" => Some(Keyword::Const),
            "test" => Some(Keyword::Test),
            "import" => Some(Keyword::Import),
            "pub" => Some(Keyword::Pub),
            "let" => Some(Keyword::Let),
            "var" => Some(Keyword::Var),
            "mut" => Some(Keyword::Mut),
            "if" => Some(Keyword::If),
            "else" => Some(Keyword::Else),
            "match" => Some(Keyword::Match),
            "when" => Some(Keyword::When),
            "for" => Some(Keyword::For),
            "in" => Some(Keyword::In),
            "while" => Some(Keyword::While),
            "loop" => Some(Keyword::Loop),
            "break" => Some(Keyword::Break),
            "continue" => Some(Keyword::Continue),
            "return" => Some(Keyword::Return),
            "fails" => Some(Keyword::Fails),
            "fail" => Some(Keyword::Fail),
            "try" => Some(Keyword::Try),
            "attempt" => Some(Keyword::Attempt),
            "requires" => Some(Keyword::Requires),
            "assert" => Some(Keyword::Assert),
            "uses" => Some(Keyword::Uses),
            "provide" => Some(Keyword::Provide),
            "with" => Some(Keyword::With),
            "where" => Some(Keyword::Where),
            "any" => Some(Keyword::Any),
            "Self" => Some(Keyword::SelfUpper),
            "self" => Some(Keyword::SelfLower),
            "and" => Some(Keyword::And),
            "or" => Some(Keyword::Or),
            "not" => Some(Keyword::Not),
            "is" => Some(Keyword::Is),
            "as" => Some(Keyword::As),
            "true" => Some(Keyword::True),
            "false" => Some(Keyword::False),
            "none" => Some(Keyword::None),
            "it" => Some(Keyword::It),
            _ => None,
        }
    }

    /// Returns the keyword's text, such as `"let"`.
    pub fn as_str(self) -> &'static str {
        match self {
            Keyword::Fn => "fn",
            Keyword::Type => "type",
            Keyword::Enum => "enum",
            Keyword::Alias => "alias",
            Keyword::Interface => "interface",
            Keyword::Impl => "impl",
            Keyword::Effect => "effect",
            Keyword::Const => "const",
            Keyword::Test => "test",
            Keyword::Import => "import",
            Keyword::Pub => "pub",
            Keyword::Let => "let",
            Keyword::Var => "var",
            Keyword::Mut => "mut",
            Keyword::If => "if",
            Keyword::Else => "else",
            Keyword::Match => "match",
            Keyword::When => "when",
            Keyword::For => "for",
            Keyword::In => "in",
            Keyword::While => "while",
            Keyword::Loop => "loop",
            Keyword::Break => "break",
            Keyword::Continue => "continue",
            Keyword::Return => "return",
            Keyword::Fails => "fails",
            Keyword::Fail => "fail",
            Keyword::Try => "try",
            Keyword::Attempt => "attempt",
            Keyword::Requires => "requires",
            Keyword::Assert => "assert",
            Keyword::Uses => "uses",
            Keyword::Provide => "provide",
            Keyword::With => "with",
            Keyword::Where => "where",
            Keyword::Any => "any",
            Keyword::SelfUpper => "Self",
            Keyword::SelfLower => "self",
            Keyword::And => "and",
            Keyword::Or => "or",
            Keyword::Not => "not",
            Keyword::Is => "is",
            Keyword::As => "as",
            Keyword::True => "true",
            Keyword::False => "false",
            Keyword::None => "none",
            Keyword::It => "it",
        }
    }
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

    /// Moves past the characters for which `predicate` holds.
    fn eat_while(&mut self, predicate: impl Fn(char) -> bool) {
        while self.first().is_some_and(&predicate) {
            self.advance();
        }
    }

    /// Returns whether the cursor is at the end of its line: on `\n`, on `\r\n`, or at the
    /// end of the text.
    fn at_line_end(&self) -> bool {
        match self.first() {
            None | Some('\n') => true,
            Some('\r') => self.second() == Some('\n'),
            Some(_) => false,
        }
    }
}

struct Lexer<'a> {
    file: FileId,
    text: &'a str,
    cursor: Cursor<'a>,
    diagnostics: Vec<Diagnostic>,
    tokens: Vec<Token>,
    /// The delimiters not closed yet, innermost last.
    open_delimiters: Vec<Open>,
    /// How many delimiters of each kind `open_delimiters` holds above the innermost
    /// interpolation, indexed by `Delimiter as usize`.
    open_counts: [usize; 3],
    /// The position of the last line end that cut a literal. That literal reported it, so
    /// the interpolations it closes report nothing more.
    reported_line_end: Option<usize>,
}

impl<'a> Lexer<'a> {
    fn new(text: &'a str, file: FileId) -> Self {
        let cursor = Cursor {
            chars: text.chars(),
            text_len: text.len(),
        };

        Self {
            file,
            text,
            cursor,
            diagnostics: Vec::new(),
            tokens: Vec::new(),
            open_delimiters: Vec::new(),
            open_counts: [0; 3],
            reported_line_end: None,
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
                    if self.cursor.at_line_end() {
                        self.close_interpolations_at_line_end();
                    }

                    if let Some(token) = self.read_newline() {
                        if self.keeps_newline() {
                            self.tokens.push(token);
                        }
                    } else {
                        // TODO(J02): refuse a lone `\r` as a forbidden character (LEX-2).
                        self.skip_unexpected(c);
                    }
                }
                // In an interpolation, `//` is two `/` (LEX-5).
                '/' if self.cursor.second() == Some('/') && !self.in_interpolation() => {
                    if let Some(token) = self.read_comment() {
                        self.tokens.push(token);
                    }
                }
                '\'' => {
                    let token = self.read_char();
                    self.tokens.push(token);
                }
                '"' => {
                    let start = self.cursor.position();
                    self.cursor.advance();
                    self.tokens
                        .push(Token::new(TokenKind::TextStart, self.span_from(start)));

                    self.read_text(start);
                }
                '}' if self.closes_interpolation() => self.read_interpolation_end(),
                // At depth zero: no delimiter is open in the innermost interpolation (LEX-20).
                ':' if matches!(self.open_delimiters.last(), Some(Open::Interpolation(_))) => {
                    let token = self.read_format();
                    self.tokens.push(token);
                }
                // Before words: they would read `r` as an identifier.
                'r' if self.cursor.second() == Some('"') => {
                    let token = self.read_raw_text();
                    self.tokens.push(token);
                }
                // ASCII only: `is_alphabetic` would also take `é` (LEX-8).
                c if c.is_ascii_alphabetic() || c == '_' => {
                    let token = self.read_word();
                    self.tokens.push(token);
                }
                c if c.is_ascii_digit() => {
                    let token = self.read_number();
                    self.tokens.push(token);
                }
                _ => {
                    if let Some(token) = self.read_operator() {
                        self.track_delimiter(token.kind());
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

        // The end of the file is also a line end.
        self.close_interpolations_at_line_end();

        self.tokens.push(Token::new(
            TokenKind::Eof,
            self.span_from(self.cursor.position()),
        ));

        Lexed {
            tokens: self.tokens,
            diagnostics: self.diagnostics,
        }
    }

    /// Reads a character literal, then checks it. A refused literal is reported, then read as
    /// U+FFFD.
    fn read_char(&mut self) -> Token {
        let start = self.cursor.position();
        self.cursor.advance();

        let mut terminated = false;

        while let Some(c) = self.cursor.first() {
            match c {
                '\'' => {
                    self.cursor.advance();
                    terminated = true;
                    break;
                }
                '\\' => {
                    self.cursor.advance();
                    if !self.cursor.at_line_end() {
                        self.cursor.advance();
                    }
                }
                _ if self.cursor.at_line_end() => {
                    break;
                }
                _ => {
                    self.cursor.advance();
                }
            }
        }

        let span = self.span_from(start);

        if !terminated {
            self.report_cut(unterminated_character_literal(span));
            return Token::new(TokenKind::Char(char::REPLACEMENT_CHARACTER), span);
        }

        // Each quote is one byte.
        let content = &self.text[start + 1..self.cursor.position() - 1];

        let (value, len) = match content.chars().next() {
            None => {
                self.diagnostics.push(empty_character_literal(span));
                return Token::new(TokenKind::Char(char::REPLACEMENT_CHARACTER), span);
            }
            Some('\\') => match escape::read(content) {
                Ok(read) => read,
                Err(error) => {
                    // The escape starts after the opening quote.
                    self.diagnostics
                        .push(error.to_diagnostic(content, self.file, start + 1));
                    return Token::new(TokenKind::Char(char::REPLACEMENT_CHARACTER), span);
                }
            },
            Some(c) => (c, c.len_utf8()),
        };

        if !content[len..].is_empty() {
            self.diagnostics
                .push(character_literal_with_more_than_one_character(
                    content, span,
                ));
            return Token::new(TokenKind::Char(char::REPLACEMENT_CHARACTER), span);
        }

        Token::new(TokenKind::Char(value), span)
    }

    /// Reads the characters of a text, after its `"` or after an interpolation, up to its
    /// closing `"`, its next `{`, or the end of the line. `text_start` is the position of the
    /// text's `"`: a text cut by the end of the line is reported from there.
    fn read_text(&mut self, text_start: usize) {
        let mut piece = String::new();
        let piece_start = self.cursor.position();

        while let Some(c) = self.cursor.first() {
            match c {
                '"' => {
                    self.push_piece(&mut piece, piece_start);

                    let start = self.cursor.position();
                    self.cursor.advance();
                    self.tokens
                        .push(Token::new(TokenKind::TextEnd, self.span_from(start)));

                    return;
                }
                '{' => {
                    self.push_piece(&mut piece, piece_start);

                    let start = self.cursor.position();
                    self.cursor.advance();
                    self.tokens.push(Token::new(
                        TokenKind::InterpolationStart,
                        self.span_from(start),
                    ));
                    self.open_interpolation(text_start, start);

                    return;
                }
                '\\' => {
                    let start = self.cursor.position();
                    let rest = self.cursor.as_str();
                    let end = match escape::read(rest) {
                        Ok((c, len)) => {
                            piece.push(c);
                            start + len
                        }
                        Err(error) => {
                            piece.push(char::REPLACEMENT_CHARACTER);
                            self.diagnostics
                                .push(error.to_diagnostic(rest, self.file, start));

                            start + error.len()
                        }
                    };

                    while self.cursor.position() < end {
                        self.cursor.advance();
                    }
                }
                _ if self.cursor.at_line_end() => break,
                // Reported, but kept in the piece.
                '}' => {
                    let start = self.cursor.position();
                    self.cursor.advance();

                    self.diagnostics
                        .push(unmatched_brace_in_text(self.span_from(start)));
                    piece.push('}');
                }
                c => {
                    piece.push(c);
                    self.cursor.advance();
                }
            }
        }

        // The line end, or the end of the file, ends the text.
        self.report_cut(unterminated_text(self.span_from(text_start)));
        self.push_piece(&mut piece, piece_start);
        self.tokens.push(Token::new(
            TokenKind::TextEnd,
            self.span_from(self.cursor.position()),
        ));
    }

    /// Pushes the piece read from `start` to the cursor, and leaves `piece` empty. An empty
    /// piece gives no token.
    fn push_piece(&mut self, piece: &mut String, start: usize) {
        if !piece.is_empty() {
            self.tokens.push(Token::new(
                TokenKind::TextPiece(std::mem::take(piece)),
                self.span_from(start),
            ));
        }
    }

    /// Reads the `}` that closes the innermost interpolation, then the rest of its text. An
    /// empty interpolation is reported.
    fn read_interpolation_end(&mut self) {
        let start = self.cursor.position();
        let interpolation = self.close_interpolation();
        self.cursor.advance();

        // `{` is one byte. Not `trim`, which also takes Unicode spaces (C.32).
        let inside = &self.text[interpolation.brace_start + 1..start];
        if inside.chars().all(|c| matches!(c, ' ' | '\t')) {
            self.diagnostics.push(empty_interpolation(
                self.span_from(interpolation.brace_start),
            ));
        }

        self.tokens.push(Token::new(
            TokenKind::InterpolationEnd,
            self.span_from(start),
        ));

        self.read_text(interpolation.text_start);
    }

    /// Reads the format of an interpolation, from its `:` to its `}` or to the end of the
    /// line. It holds what follows the `:`, as written.
    fn read_format(&mut self) -> Token {
        let start = self.cursor.position();

        while !self.cursor.at_line_end() && self.cursor.first() != Some('}') {
            self.cursor.advance();
        }

        // `:` is one byte.
        let content = &self.text[start + 1..self.cursor.position()];
        Token::new(
            TokenKind::Format(content.to_string()),
            self.span_from(start),
        )
    }

    /// Reads a raw text, up to its closing `"` on the same line. An unterminated raw text is
    /// reported, then read with what it holds up to the end of the line.
    fn read_raw_text(&mut self) -> Token {
        let start = self.cursor.position();
        self.cursor.advance();
        self.cursor.advance();

        while !self.cursor.at_line_end() && self.cursor.first() != Some('"') {
            self.cursor.advance();
        }

        // `r` and `"` are one byte each.
        let content = self.text[start + 2..self.cursor.position()].to_string();

        if self.cursor.first() == Some('"') {
            self.cursor.advance();
        } else {
            self.report_cut(unterminated_raw_text(self.span_from(start)));
        }

        Token::new(TokenKind::RawText(content), self.span_from(start))
    }

    /// Reads a word of ASCII letters, digits and `_`, then tells what it is: `_` alone, a
    /// keyword or an identifier. A reserved word is reported, then read as an identifier.
    fn read_word(&mut self) -> Token {
        let start = self.cursor.position();

        // The whole word first: deciding earlier would cut `letter` into `let` and `ter`.
        self.cursor
            .eat_while(|c| c.is_ascii_alphanumeric() || c == '_');

        let span = self.span_from(start);
        let text = &self.text[start..self.cursor.position()];

        let kind = match text {
            "_" => TokenKind::Underscore,
            // Read as an identifier, so that the parser does not report it a second time.
            _ if RESERVED_WORDS.contains(&text) => {
                self.diagnostics.push(reserved_word(text, span));
                TokenKind::Identifier
            }
            _ => Keyword::from_text(text).map_or(TokenKind::Identifier, TokenKind::Keyword),
        };

        Token::new(kind, span)
    }

    /// Reads a number literal, then checks it. A refused literal is reported, then read with a
    /// placeholder value.
    fn read_number(&mut self) -> Token {
        let start = self.cursor.position();
        // Only a decimal literal has an exponent, so in `0x1e+5` and `0b1e+1` the `+` ends
        // the literal.
        let is_decimal = number::starts_decimal(self.cursor.as_str());

        // The literal goes past a `.` or a sign only when a digit follows: `1..10` and
        // `5.abs()` stop before the `.`.
        loop {
            self.cursor
                .eat_while(|c| c.is_ascii_alphabetic() || c.is_ascii_digit() || c == '_');
            let read = &self.text[start..self.cursor.position()];

            match self.cursor.first() {
                Some('.')
                    if self.cursor.second().is_some_and(|c| c.is_ascii_digit())
                        && !read.contains('.') =>
                {
                    self.cursor.advance();
                }
                Some('+' | '-')
                    if self.cursor.second().is_some_and(|c| c.is_ascii_digit())
                        && read.ends_with(['e', 'E'])
                        && is_decimal =>
                {
                    self.cursor.advance();
                }
                _ => break,
            }
        }

        let text = &self.text[start..self.cursor.position()];
        let kind = match number::validate_number(text) {
            Ok(kind) => kind,
            Err(error) => {
                self.diagnostics
                    .push(error.to_diagnostic(text, self.file, start));
                error.placeholder()
            }
        };

        Token::new(kind, self.span_from(start))
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

    /// Reads a comment, from `//` to the end of the line.
    ///
    /// Only a doc comment alone on its line gives a token. Any other doc comment is reported.
    fn read_comment(&mut self) -> Option<Token> {
        let start = self.cursor.position();

        self.cursor.advance();
        self.cursor.advance();
        let is_doc = self.cursor.first() == Some('/') && self.cursor.second() != Some('/');
        self.skip_until_newline();

        if !is_doc {
            return None;
        }

        let span = self.span_from(start);
        let before = &self.text[..start];
        let line_before = before.rsplit_once('\n').map_or(before, |(_, line)| line);

        if !line_before.chars().all(|c| matches!(c, ' ' | '\t')) {
            self.diagnostics
                .push(doc_comment_not_alone_on_its_line(span));
            return None;
        }

        let text = &self.text[start + "///".len()..self.cursor.position()];
        let text = text.strip_prefix(' ').unwrap_or(text);

        Some(Token::new(TokenKind::DocComment(text.to_string()), span))
    }

    /// Skips the rest of the line, up to its line ending.
    fn skip_until_newline(&mut self) {
        while !self.cursor.at_line_end() {
            self.cursor.advance();
        }
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

    /// Returns whether a newline read now gives a token: not inside `(` or `[`, and not right
    /// after another newline.
    fn keeps_newline(&self) -> bool {
        let inside_parens_or_brackets = matches!(
            self.open_delimiters.last(),
            Some(Open::Delimiter(Delimiter::Paren | Delimiter::Bracket))
        );
        let after_newline = matches!(
            self.tokens.last().map(Token::kind),
            Some(TokenKind::Newline)
        );

        !inside_parens_or_brackets && !after_newline
    }

    /// Updates the open delimiters after a token of kind `kind`.
    fn track_delimiter(&mut self, kind: &TokenKind) {
        match kind {
            TokenKind::OpenParen => self.open(Delimiter::Paren),
            TokenKind::OpenBracket => self.open(Delimiter::Bracket),
            TokenKind::OpenBrace => self.open(Delimiter::Brace),
            TokenKind::CloseParen => self.close(Delimiter::Paren),
            TokenKind::CloseBracket => self.close(Delimiter::Bracket),
            TokenKind::CloseBrace => self.close(Delimiter::Brace),
            _ => {}
        }
    }

    fn open(&mut self, delimiter: Delimiter) {
        self.open_delimiters.push(Open::Delimiter(delimiter));
        self.open_counts[delimiter as usize] += 1;
    }

    /// Returns how many `delimiter` are open above the innermost interpolation.
    fn open_count(&self, delimiter: Delimiter) -> usize {
        self.open_counts[delimiter as usize]
    }

    /// Closes `delimiter`, and any delimiter left open inside it.
    ///
    /// Without an open `delimiter`, nothing changes: a missing `)` does not turn every later
    /// line ending into a space. The counts tell it at once; a search through the stack would
    /// make `((((…]]]]` quadratic.
    fn close(&mut self, delimiter: Delimiter) {
        if self.open_count(delimiter) == 0 {
            return;
        }

        while let Some(open) = self.open_delimiters.pop() {
            let Open::Delimiter(open) = open else {
                unreachable!("the counts only see the delimiters above the innermost interpolation")
            };

            self.open_counts[open as usize] -= 1;
            if open == delimiter {
                break;
            }
        }
    }

    /// Returns whether an interpolation is open.
    fn in_interpolation(&self) -> bool {
        // Every counted delimiter lies above the innermost interpolation, so the stack holds
        // more than the counts only when an interpolation is open.
        self.open_delimiters.len() > self.open_counts.iter().sum::<usize>()
    }

    /// Returns whether a `}` read now closes an interpolation: one is open, and no `{` is left
    /// open inside it.
    fn closes_interpolation(&self) -> bool {
        self.in_interpolation() && self.open_count(Delimiter::Brace) == 0
    }

    fn open_interpolation(&mut self, text_start: usize, brace_start: usize) {
        // A delimiter inside the interpolation cannot close one outside it. So the counts
        // start again from zero, and the outer ones come back when it closes.
        let outer_counts = std::mem::take(&mut self.open_counts);

        self.open_delimiters
            .push(Open::Interpolation(Interpolation {
                text_start,
                brace_start,
                outer_counts,
            }));
    }

    /// Closes the innermost interpolation, and any delimiter left open inside it, then
    /// returns it.
    fn close_interpolation(&mut self) -> Interpolation {
        while let Some(open) = self.open_delimiters.pop() {
            if let Open::Interpolation(interpolation) = open {
                self.open_counts = interpolation.outer_counts;
                return interpolation;
            }
        }

        unreachable!("the caller checked that an interpolation is open")
    }

    /// Reports a literal cut by the end of its line, and remembers that this line end is
    /// reported.
    fn report_cut(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
        self.reported_line_end = Some(self.cursor.position());
    }

    /// Closes each interpolation still open, with its text: neither holds a line ending.
    /// Their end tokens are empty, at the line end.
    ///
    /// The line end is reported once (DIA-3): by the literal it cuts, if any, or else by the
    /// innermost interpolation.
    fn close_interpolations_at_line_end(&mut self) {
        let mut reported = self.reported_line_end == Some(self.cursor.position());

        while self.in_interpolation() {
            let interpolation = self.close_interpolation();

            if !reported {
                self.diagnostics.push(unterminated_interpolation(
                    self.span_from(interpolation.brace_start),
                ));
                reported = true;
            }

            let end = self.span_from(self.cursor.position());
            self.tokens
                .push(Token::new(TokenKind::InterpolationEnd, end));
            self.tokens.push(Token::new(TokenKind::TextEnd, end));
        }
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
/// - A line ending, `\n` or `\r\n`, gives a newline token, unless the innermost open
///   delimiter is `(` or `[`. Consecutive newline tokens merge into the first.
/// - A comment runs from `//` to the end of the line, and gives no token.
/// - A doc comment, `///`, gives a token if it is alone on its line, and a diagnostic
///   otherwise.
/// - A word of ASCII letters, digits and `_`, not starting with a digit, is read whole: `let`
///   is a keyword, and `letter` an identifier. `_` alone is the wildcard. A reserved word,
///   such as `yield`, gets a diagnostic, then gives an identifier.
/// - A number literal starts with a digit, and runs over the ASCII letters, digits and `_`
///   that follow. It goes past a `.`, or past a sign after the `e` of a decimal literal,
///   only before a digit: `1..10` gives `1`, `..` and `10`. A refused literal gets a
///   diagnostic, then gives a token of its kind.
/// - A character literal runs from `'` to the next `'` of the line that no `\` escapes, or
///   to the end of the line. It holds one character, written as it is or as an escape:
///   `'a'`, `'\n'`, `'\u{1F600}'`. A refused literal gets a diagnostic, then gives U+FFFD.
/// - A raw text, `r` right before `"`, runs to the next `"` of the line, and keeps each
///   character as written. Without that `"`, it gets a diagnostic, and holds the rest of
///   the line.
/// - A text, from `"` to the next `"` of the line, gives `TextStart`, its pieces and its
///   interpolations, then `TextEnd`. A piece holds its characters with their escapes read.
///   An interpolation, from `{` to its `}`, is read as code, where `//` is two `/`; its
///   first `:` outside any `(`, `[` or `{` of that code starts its format. A text or an
///   interpolation not closed on its line gets a diagnostic, and ends at the end of the line.
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
/// A line gives the token's start as `line:column`, then its kind, with its value if it has
/// one, and its text, both written by `Debug`:
///
/// ```text
/// 1:1 DocComment("Sums.") "/// Sums."
/// 1:10 Newline "\n"
/// 2:1 Plus "+"
/// 2:3 ShlEq "<<="
/// 2:6 Eof ""
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

/// Writes the content of a character literal as the content of a text. A text gives a
/// meaning to `"`, `{` and `}`, so each one gets a `\`, unless it is part of an escape.
fn escape_for_text(content: &str) -> String {
    let mut text = String::with_capacity(content.len());
    let mut rest = content;

    while let Some(c) = rest.chars().next() {
        let len = match c {
            // Copied as written, refused or not: `{` is part of `\u{41}`.
            '\\' => escape::read(rest).map_or_else(|error| error.len(), |(_, len)| len),
            '"' | '{' | '}' => {
                text.push('\\');
                1
            }
            _ => c.len_utf8(),
        };
        text.push_str(&rest[..len]);
        rest = &rest[len..];
    }

    text
}

fn usize_to_u32(value: usize) -> u32 {
    u32::try_from(value).expect("SourceMap::add() checked that offsets fit in u32")
}

#[cfg(test)]
mod tests;
