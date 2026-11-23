//! The lexer, which splits a file's text into tokens.

mod number;

use std::fmt::Write;
use std::str::Chars;

use frame_diagnostics::{Diagnostic, Prefix, Primary, RuleCode};
use frame_source::{FileId, SourceFile, SourceMap, Span};

const UNEXPECTED_CHARACTER: RuleCode = RuleCode::new(Prefix::Lex, 24);
const DOC_COMMENT_NOT_ALONE_ON_ITS_LINE: RuleCode = RuleCode::new(Prefix::Lex, 6);
const RESERVED_WORD: RuleCode = RuleCode::new(Prefix::Lex, 11);

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
    /// todo!
    Int(Option<u64>),
    /// todo!
    Float(String),
    /// `_` alone, the wildcard of patterns. `_x` is an identifier.
    Underscore,
    /// A line ending: `\n` or `\r\n`.
    Newline,
    /// The end of the text. Its span is empty.
    Eof,
}

/// A delimiter that waits for its closing token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Open {
    Paren,
    Bracket,
    Brace,
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
}

struct Lexer<'a> {
    file: FileId,
    text: &'a str,
    cursor: Cursor<'a>,
    diagnostics: Vec<Diagnostic>,
    tokens: Vec<Token>,
    /// The delimiters not closed yet, innermost last.
    open_delimiters: Vec<Open>,
    /// How many delimiters of each kind `open_delimiters` holds, indexed by `Open as usize`.
    open_counts: [usize; 3],
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
                        if self.keeps_newline() {
                            self.tokens.push(token);
                        }
                    } else {
                        // TODO(J02): refuse a lone `\r` as a forbidden character (LEX-2).
                        self.skip_unexpected(c);
                    }
                }
                '/' if self.cursor.second() == Some('/') => {
                    if let Some(token) = self.read_comment() {
                        self.tokens.push(token);
                    }
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

        self.tokens.push(Token::new(
            TokenKind::Eof,
            self.span_from(self.cursor.position()),
        ));

        Lexed {
            tokens: self.tokens,
            diagnostics: self.diagnostics,
        }
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

    fn read_number(&mut self) -> Token {
        let start = self.cursor.position();
        let is_decimal = !(self.cursor.first() == Some('0')
            && matches!(
                self.cursor.second(),
                Some('x' | 'o' | 'b' | 'X' | 'O' | 'B')
            ));

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

        let _text = &self.text[start..self.cursor.position()];
        Token::new(TokenKind::Int(Some(0)), self.span_from(start))
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
        while let Some(c) = self.cursor.first() {
            if c == '\n' || (c == '\r' && self.cursor.second() == Some('\n')) {
                break;
            }
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
            Some(Open::Paren | Open::Bracket)
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
            TokenKind::OpenParen => self.open(Open::Paren),
            TokenKind::OpenBracket => self.open(Open::Bracket),
            TokenKind::OpenBrace => self.open(Open::Brace),
            TokenKind::CloseParen => self.close(Open::Paren),
            TokenKind::CloseBracket => self.close(Open::Bracket),
            TokenKind::CloseBrace => self.close(Open::Brace),
            _ => {}
        }
    }

    fn open(&mut self, delimiter: Open) {
        self.open_delimiters.push(delimiter);
        self.open_counts[delimiter as usize] += 1;
    }

    /// Closes `delimiter`, and any delimiter left open inside it.
    ///
    /// Without an open `delimiter`, nothing changes: a missing `)` does not turn every later
    /// line ending into a space. The counts tell it at once; a search through the stack would
    /// make `((((…]]]]` quadratic.
    fn close(&mut self, delimiter: Open) {
        if self.open_counts[delimiter as usize] == 0 {
            return;
        }

        while let Some(open) = self.open_delimiters.pop() {
            self.open_counts[open as usize] -= 1;
            if open == delimiter {
                break;
            }
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

fn usize_to_u32(value: usize) -> u32 {
    u32::try_from(value).expect("SourceMap::add() checked that offsets fit in u32")
}

#[cfg(test)]
mod tests {
    use super::{Keyword, dump_tokens, tokenize};
    use frame_diagnostics::{Diagnostic, Prefix, Primary, RuleCode};
    use frame_source::{FileId, SourceMap, Span};
    use std::ops::Range;

    use ExpectedDiagnostic::{DocCommentNotAlone, ReservedWord, UnexpectedCharacter};

    /// A diagnostic the lexer should report, with its byte range.
    ///
    /// Titles and helps are written out here, apart from the lexer's code.
    enum ExpectedDiagnostic {
        UnexpectedCharacter(char, Range<u32>),
        DocCommentNotAlone(Range<u32>),
        ReservedWord(&'static str, Range<u32>),
    }

    impl ExpectedDiagnostic {
        fn to_diagnostic(&self, file: FileId) -> Diagnostic {
            match self {
                UnexpectedCharacter(c, range) => Diagnostic::new(
                    RuleCode::new(Prefix::Lex, 24),
                    format!("unexpected character `{c}`"),
                    Primary::span(Span::new(file, range.start, range.end)),
                ),
                DocCommentNotAlone(range) => Diagnostic::new(
                    RuleCode::new(Prefix::Lex, 6),
                    "doc comment is not alone on its line",
                    Primary::span(Span::new(file, range.start, range.end)),
                )
                .with_help("for an ordinary comment, write `//`"),
                ReservedWord(word, range) => Diagnostic::new(
                    RuleCode::new(Prefix::Lex, 11),
                    format!("`{word}` is reserved"),
                    Primary::span(Span::new(file, range.start, range.end)),
                )
                .with_help(
                    "this word is reserved for a future version of Frame; choose another name",
                ),
            }
        }
    }

    /// Tokenizes `text`, alone in a source map, and compares the result in full.
    ///
    /// `expected_tokens` holds one line of `dump_tokens` per token, without its `\n`.
    /// `expected_diagnostics` holds the diagnostics in text order.
    fn assert_lexed(
        text: &str,
        expected_tokens: &[&str],
        expected_diagnostics: &[ExpectedDiagnostic],
    ) {
        let mut source_map = SourceMap::new();
        let file = source_map.add(String::from("test.fm"), text.to_string());
        let lexed = tokenize(&source_map, file);

        let mut expected_dump = String::new();
        for line in expected_tokens {
            expected_dump.push_str(line);
            expected_dump.push('\n');
        }
        let expected_diagnostics: Vec<Diagnostic> = expected_diagnostics
            .iter()
            .map(|expected| expected.to_diagnostic(file))
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

    /// Checks that `text` gives one token, of kind `kind`, and no diagnostic.
    ///
    /// `text` is ASCII, so its length gives the end's column.
    fn assert_one_token(text: &str, kind: &str) {
        let token = format!("1:1 {kind} {text:?}");
        let end = format!(r#"1:{} Eof """#, text.len() + 1);
        assert_lexed(text, &[&token, &end], &[]);
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
            assert_one_token(text, kind);
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
        assert_lexed("@", &[r#"1:2 Eof """#], &[UnexpectedCharacter('@', 0..1)]);
    }

    #[test]
    fn unexpected_character_covers_all_its_bytes() {
        assert_lexed(
            "+é-",
            &[r#"1:1 Plus "+""#, r#"1:3 Minus "-""#, r#"1:4 Eof """#],
            &[UnexpectedCharacter('é', 1..3)],
        );
    }

    #[test]
    fn every_unexpected_character_is_reported_in_text_order() {
        assert_lexed(
            "@ $",
            &[r#"1:4 Eof """#],
            &[
                UnexpectedCharacter('@', 0..1),
                UnexpectedCharacter('$', 2..3),
            ],
        );
    }

    #[test]
    fn lone_carriage_return_is_an_unexpected_character() {
        assert_lexed(
            "+\r-",
            &[r#"1:1 Plus "+""#, r#"1:3 Minus "-""#, r#"1:4 Eof """#],
            &[UnexpectedCharacter('\r', 1..2)],
        );
    }

    #[test]
    fn no_break_space_is_an_unexpected_character() {
        // Only the space and the tab separate tokens (LEX-4).
        assert_lexed(
            "+\u{A0}-",
            &[r#"1:1 Plus "+""#, r#"1:3 Minus "-""#, r#"1:4 Eof """#],
            &[UnexpectedCharacter('\u{A0}', 1..3)],
        );
    }

    #[test]
    fn consecutive_line_endings_merge_into_the_first() {
        assert_lexed(
            "+\n\n\n-",
            &[
                r#"1:1 Plus "+""#,
                r#"1:2 Newline "\n""#,
                r#"4:1 Minus "-""#,
                r#"4:2 Eof """#,
            ],
            &[],
        );
    }

    #[test]
    fn comment_gives_no_token() {
        let cases: [(&str, &[&str]); 2] = [
            (
                "+\n  // note\n-",
                &[
                    r#"1:1 Plus "+""#,
                    r#"1:2 Newline "\n""#,
                    r#"3:1 Minus "-""#,
                    r#"3:2 Eof """#,
                ],
            ),
            ("+ // note", &[r#"1:1 Plus "+""#, r#"1:10 Eof """#]),
        ];

        for (text, expected_tokens) in cases {
            assert_lexed(text, expected_tokens, &[]);
        }
    }

    #[test]
    fn line_ending_after_a_comment_keeps_its_token() {
        assert_lexed(
            "+ // note\r\n-",
            &[
                r#"1:1 Plus "+""#,
                r#"1:10 Newline "\r\n""#,
                r#"2:1 Minus "-""#,
                r#"2:2 Eof """#,
            ],
            &[],
        );
    }

    #[test]
    fn lone_carriage_return_does_not_end_a_comment() {
        // TODO(J02): expect LEX-2 on the `\r`.
        let cases: [(&str, &[&str]); 2] = [
            (
                "+ // a\r-\n+",
                &[
                    r#"1:1 Plus "+""#,
                    r#"1:9 Newline "\n""#,
                    r#"2:1 Plus "+""#,
                    r#"2:2 Eof """#,
                ],
            ),
            (
                "/// a\r-\n+",
                &[
                    r#"1:1 DocComment("a\r-") "/// a\r-""#,
                    r#"1:8 Newline "\n""#,
                    r#"2:1 Plus "+""#,
                    r#"2:2 Eof """#,
                ],
            ),
        ];

        for (text, expected_tokens) in cases {
            assert_lexed(text, expected_tokens, &[]);
        }
    }

    #[test]
    fn line_endings_inside_parentheses_or_brackets_give_no_token() {
        let cases: [(&str, &[&str]); 3] = [
            (
                "(+\n-)",
                &[
                    r#"1:1 OpenParen "(""#,
                    r#"1:2 Plus "+""#,
                    r#"2:1 Minus "-""#,
                    r#"2:2 CloseParen ")""#,
                    r#"2:3 Eof """#,
                ],
            ),
            (
                "[+\n-]",
                &[
                    r#"1:1 OpenBracket "[""#,
                    r#"1:2 Plus "+""#,
                    r#"2:1 Minus "-""#,
                    r#"2:2 CloseBracket "]""#,
                    r#"2:3 Eof """#,
                ],
            ),
            (
                "(()\n)",
                &[
                    r#"1:1 OpenParen "(""#,
                    r#"1:2 OpenParen "(""#,
                    r#"1:3 CloseParen ")""#,
                    r#"2:1 CloseParen ")""#,
                    r#"2:2 Eof """#,
                ],
            ),
        ];

        for (text, expected_tokens) in cases {
            assert_lexed(text, expected_tokens, &[]);
        }
    }

    #[test]
    fn line_ending_inside_braces_gives_a_token() {
        assert_lexed(
            "{+\n-}",
            &[
                r#"1:1 OpenBrace "{""#,
                r#"1:2 Plus "+""#,
                r#"1:3 Newline "\n""#,
                r#"2:1 Minus "-""#,
                r#"2:2 CloseBrace "}""#,
                r#"2:3 Eof """#,
            ],
            &[],
        );
    }

    #[test]
    fn innermost_open_delimiter_decides() {
        let cases: [(&str, &[&str]); 2] = [
            (
                "({\n})",
                &[
                    r#"1:1 OpenParen "(""#,
                    r#"1:2 OpenBrace "{""#,
                    r#"1:3 Newline "\n""#,
                    r#"2:1 CloseBrace "}""#,
                    r#"2:2 CloseParen ")""#,
                    r#"2:3 Eof """#,
                ],
            ),
            (
                "{(\n)}",
                &[
                    r#"1:1 OpenBrace "{""#,
                    r#"1:2 OpenParen "(""#,
                    r#"2:1 CloseParen ")""#,
                    r#"2:2 CloseBrace "}""#,
                    r#"2:3 Eof """#,
                ],
            ),
        ];

        for (text, expected_tokens) in cases {
            assert_lexed(text, expected_tokens, &[]);
        }
    }

    #[test]
    fn line_ending_at_the_start_gives_a_token() {
        assert_lexed(
            "\n+",
            &[r#"1:1 Newline "\n""#, r#"2:1 Plus "+""#, r#"2:2 Eof """#],
            &[],
        );
    }

    #[test]
    fn closing_delimiter_without_its_opening_is_ignored() {
        let cases: [(&str, &[&str]); 3] = [
            (
                ")\n+",
                &[
                    r#"1:1 CloseParen ")""#,
                    r#"1:2 Newline "\n""#,
                    r#"2:1 Plus "+""#,
                    r#"2:2 Eof """#,
                ],
            ),
            (
                "(]\n+",
                &[
                    r#"1:1 OpenParen "(""#,
                    r#"1:2 CloseBracket "]""#,
                    r#"2:1 Plus "+""#,
                    r#"2:2 Eof """#,
                ],
            ),
            (
                "[())\n]",
                &[
                    r#"1:1 OpenBracket "[""#,
                    r#"1:2 OpenParen "(""#,
                    r#"1:3 CloseParen ")""#,
                    r#"1:4 CloseParen ")""#,
                    r#"2:1 CloseBracket "]""#,
                    r#"2:2 Eof """#,
                ],
            ),
        ];

        for (text, expected_tokens) in cases {
            assert_lexed(text, expected_tokens, &[]);
        }
    }

    #[test]
    fn closing_delimiter_also_closes_the_delimiters_inside() {
        assert_lexed(
            "([)\n+",
            &[
                r#"1:1 OpenParen "(""#,
                r#"1:2 OpenBracket "[""#,
                r#"1:3 CloseParen ")""#,
                r#"1:4 Newline "\n""#,
                r#"2:1 Plus "+""#,
                r#"2:2 Eof """#,
            ],
            &[],
        );
    }

    #[test]
    fn doc_comment_gives_a_token_with_its_text() {
        let cases: [(&str, &[&str]); 4] = [
            (
                "/// Sums.\n+",
                &[
                    r#"1:1 DocComment("Sums.") "/// Sums.""#,
                    r#"1:10 Newline "\n""#,
                    r#"2:1 Plus "+""#,
                    r#"2:2 Eof """#,
                ],
            ),
            (
                "///Sums.",
                &[r#"1:1 DocComment("Sums.") "///Sums.""#, r#"1:9 Eof """#],
            ),
            (
                "///  Sums.",
                &[r#"1:1 DocComment(" Sums.") "///  Sums.""#, r#"1:11 Eof """#],
            ),
            (
                "/// Sums.\r\n+",
                &[
                    r#"1:1 DocComment("Sums.") "/// Sums.""#,
                    r#"1:10 Newline "\r\n""#,
                    r#"2:1 Plus "+""#,
                    r#"2:2 Eof """#,
                ],
            ),
        ];

        for (text, expected_tokens) in cases {
            assert_lexed(text, expected_tokens, &[]);
        }
    }

    #[test]
    fn doc_comment_alone_on_its_line_gives_a_token() {
        let cases: [(&str, &[&str]); 2] = [
            (
                "\t  /// Sums.",
                &[r#"1:4 DocComment("Sums.") "/// Sums.""#, r#"1:13 Eof """#],
            ),
            (
                "+\n/// Sums.",
                &[
                    r#"1:1 Plus "+""#,
                    r#"1:2 Newline "\n""#,
                    r#"2:1 DocComment("Sums.") "/// Sums.""#,
                    r#"2:10 Eof """#,
                ],
            ),
        ];

        for (text, expected_tokens) in cases {
            assert_lexed(text, expected_tokens, &[]);
        }
    }

    #[test]
    fn four_slashes_open_an_ordinary_comment() {
        assert_lexed(
            "//// Sums.\n+",
            &[r#"1:11 Newline "\n""#, r#"2:1 Plus "+""#, r#"2:2 Eof """#],
            &[],
        );
    }

    #[test]
    fn doc_comment_not_alone_on_its_line_is_reported_and_skipped() {
        assert_lexed(
            "+ /// Sums.",
            &[r#"1:1 Plus "+""#, r#"1:12 Eof """#],
            &[DocCommentNotAlone(2..11)],
        );
        // Only spaces and tabs may come before it, not every Unicode space.
        assert_lexed(
            "\u{A0}/// Sums.",
            &[r#"1:11 Eof """#],
            &[
                UnexpectedCharacter('\u{A0}', 0..2),
                DocCommentNotAlone(2..11),
            ],
        );
    }

    #[test]
    fn each_keyword_is_recognized_and_gives_back_its_text() {
        // Written from the specification's table, in its order, not from the lexer's.
        let keywords: [(&str, Keyword); 47] = [
            ("fn", Keyword::Fn),
            ("type", Keyword::Type),
            ("enum", Keyword::Enum),
            ("alias", Keyword::Alias),
            ("interface", Keyword::Interface),
            ("impl", Keyword::Impl),
            ("effect", Keyword::Effect),
            ("const", Keyword::Const),
            ("test", Keyword::Test),
            ("import", Keyword::Import),
            ("pub", Keyword::Pub),
            ("let", Keyword::Let),
            ("var", Keyword::Var),
            ("mut", Keyword::Mut),
            ("if", Keyword::If),
            ("else", Keyword::Else),
            ("match", Keyword::Match),
            ("when", Keyword::When),
            ("for", Keyword::For),
            ("in", Keyword::In),
            ("while", Keyword::While),
            ("loop", Keyword::Loop),
            ("break", Keyword::Break),
            ("continue", Keyword::Continue),
            ("return", Keyword::Return),
            ("fails", Keyword::Fails),
            ("fail", Keyword::Fail),
            ("try", Keyword::Try),
            ("attempt", Keyword::Attempt),
            ("requires", Keyword::Requires),
            ("assert", Keyword::Assert),
            ("uses", Keyword::Uses),
            ("provide", Keyword::Provide),
            ("with", Keyword::With),
            ("where", Keyword::Where),
            ("any", Keyword::Any),
            ("Self", Keyword::SelfUpper),
            ("and", Keyword::And),
            ("or", Keyword::Or),
            ("not", Keyword::Not),
            ("is", Keyword::Is),
            ("as", Keyword::As),
            ("true", Keyword::True),
            ("false", Keyword::False),
            ("none", Keyword::None),
            ("self", Keyword::SelfLower),
            ("it", Keyword::It),
        ];

        for (text, keyword) in keywords {
            assert_one_token(text, &format!("Keyword({keyword:?})"));
            assert_eq!(keyword.as_str(), text, "text of {keyword:?}");
        }
    }

    #[test]
    fn word_that_contains_a_keyword_is_an_identifier() {
        for text in ["letter", "fnord", "xfn", "if_", "self2"] {
            assert_one_token(text, "Identifier");
        }
    }

    #[test]
    fn keywords_match_case_exactly() {
        let cases = [
            ("Fn", "Identifier"),
            ("SELF", "Identifier"),
            ("True", "Identifier"),
            ("Yield", "Identifier"),
            ("self", "Keyword(SelfLower)"),
            ("Self", "Keyword(SelfUpper)"),
        ];

        for (text, kind) in cases {
            assert_one_token(text, kind);
        }
    }

    #[test]
    fn identifier_takes_letters_digits_and_underscores() {
        for text in ["x", "_x", "__", "x_1", "MAX_LEN", "HttpClient"] {
            assert_one_token(text, "Identifier");
        }
    }

    #[test]
    fn digit_does_not_start_an_identifier() {
        // TODO(J02): T-015 reads `1` as a number.
        assert_lexed(
            "1x",
            &[r#"1:2 Identifier "x""#, r#"1:3 Eof """#],
            &[UnexpectedCharacter('1', 0..1)],
        );
    }

    #[test]
    fn lone_underscore_is_the_wildcard() {
        assert_one_token("_", "Underscore");
        assert_lexed(
            "(_)",
            &[
                r#"1:1 OpenParen "(""#,
                r#"1:2 Underscore "_""#,
                r#"1:3 CloseParen ")""#,
                r#"1:4 Eof """#,
            ],
            &[],
        );
    }

    #[test]
    fn reserved_word_is_reported_then_read_as_an_identifier() {
        let reserved_words: [(&str, Range<u32>); 8] = [
            ("realize", 0..7),
            ("flow", 0..4),
            ("under", 0..5),
            ("unsafe", 0..6),
            ("extern", 0..6),
            ("comptime", 0..8),
            ("yield", 0..5),
            ("export", 0..6),
        ];

        for (word, range) in reserved_words {
            let token = format!("1:1 Identifier {word:?}");
            let end = format!(r#"1:{} Eof """#, word.len() + 1);
            assert_lexed(word, &[&token, &end], &[ReservedWord(word, range)]);
        }
        assert_lexed(
            "let yield = x",
            &[
                r#"1:1 Keyword(Let) "let""#,
                r#"1:5 Identifier "yield""#,
                r#"1:11 Eq "=""#,
                r#"1:13 Identifier "x""#,
                r#"1:14 Eof """#,
            ],
            &[ReservedWord("yield", 4..9)],
        );
    }

    #[test]
    fn nothing_is_an_identifier_even_after_uses() {
        // The parser reads it as a keyword after `uses` (LEX-12).
        assert_lexed(
            "uses nothing",
            &[
                r#"1:1 Keyword(Uses) "uses""#,
                r#"1:6 Identifier "nothing""#,
                r#"1:13 Eof """#,
            ],
            &[],
        );
    }

    #[test]
    fn statement_is_split_into_words_and_operators() {
        assert_lexed(
            "let total = price * quantity",
            &[
                r#"1:1 Keyword(Let) "let""#,
                r#"1:5 Identifier "total""#,
                r#"1:11 Eq "=""#,
                r#"1:13 Identifier "price""#,
                r#"1:19 Star "*""#,
                r#"1:21 Identifier "quantity""#,
                r#"1:29 Eof """#,
            ],
            &[],
        );
    }

    #[test]
    fn non_ascii_letter_ends_an_identifier() {
        // TODO(J02): T-020 decides how a non-ASCII letter inside a word is reported.
        assert_lexed(
            "café",
            &[r#"1:1 Identifier "caf""#, r#"1:5 Eof """#],
            &[UnexpectedCharacter('é', 3..5)],
        );
    }
}
