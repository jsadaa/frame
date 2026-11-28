use super::{Keyword, dump_tokens, tokenize};
use frame_diagnostics::{Diagnostic, Fix, Prefix, Primary, RuleCode};
use frame_source::{FileId, SourceMap, Span};
use std::ops::Range;

use ExpectedDiagnostic::{
    AfterExponent, DocCommentNotAlone, EmptyCharacterLiteral, EmptyInterpolation, FloatNotDecimal,
    InvalidDigit, LeadingZero, MalformedUnicodeEscape, MisplacedUnderscore, MissingDigits,
    MoreThanOneCharacter, NotScalarValue, NumberSuffix, ReservedWord, UnexpectedCharacter,
    UnknownEscape, UnmatchedBraceInText, UnterminatedCharacterLiteral, UnterminatedInterpolation,
    UnterminatedRawText, UnterminatedText, UppercasePrefix,
};

/// A diagnostic the lexer should report, with its byte range.
///
/// Titles and helps are written out here, apart from the lexer's code. A `u32` before
/// the range is the rule's number, when it can be LEX-13 or LEX-14.
enum ExpectedDiagnostic {
    UnexpectedCharacter(char, Range<u32>),
    DocCommentNotAlone(Range<u32>),
    ReservedWord(&'static str, Range<u32>),
    /// The help, which differs when a suggestion would not be a valid literal.
    LeadingZero(&'static str, Range<u32>),
    MisplacedUnderscore(u32, Range<u32>),
    /// The prefix as written, then as the fix writes it.
    UppercasePrefix(&'static str, &'static str, Range<u32>),
    /// The digit, then the base's name with its article, such as `an octal`.
    InvalidDigit(char, &'static str, Range<u32>),
    MissingDigits(&'static str, Range<u32>),
    NumberSuffix(u32, &'static str, Range<u32>),
    /// What follows the exponent.
    AfterExponent(&'static str, Range<u32>),
    FloatNotDecimal(Range<u32>),
    UnterminatedCharacterLiteral(Range<u32>),
    EmptyCharacterLiteral(Range<u32>),
    /// What the help writes between the quotes of a text.
    MoreThanOneCharacter(&'static str, Range<u32>),
    /// The escape as written.
    UnknownEscape(&'static str, Range<u32>),
    MalformedUnicodeEscape(Range<u32>),
    /// The escape as written.
    NotScalarValue(&'static str, Range<u32>),
    UnterminatedRawText(Range<u32>),
    UnterminatedText(Range<u32>),
    UnmatchedBraceInText(Range<u32>),
    EmptyInterpolation(Range<u32>),
    UnterminatedInterpolation(Range<u32>),
}

impl ExpectedDiagnostic {
    fn to_diagnostic(&self, file: FileId) -> Diagnostic {
        let span = |range: &Range<u32>| Span::new(file, range.start, range.end);
        let lex = |number| RuleCode::new(Prefix::Lex, number);

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
            .with_help("this word is reserved for a future version of Frame; choose another name"),
            LeadingZero(help, range) => Diagnostic::new(
                lex(13),
                "number literal with a leading zero",
                Primary::span(span(range)),
            )
            .with_help(*help),
            MisplacedUnderscore(rule, range) => Diagnostic::new(
                lex(*rule),
                "`_` must be between two digits",
                Primary::span(span(range)),
            ),
            UppercasePrefix(prefix, fix, range) => Diagnostic::new(
                lex(13),
                format!("uppercase prefix `{prefix}`"),
                Primary::span(span(range)),
            )
            .with_help("prefixes are lowercase")
            .with_fix(Fix::new(span(range), *fix)),
            InvalidDigit(digit, base, range) => Diagnostic::new(
                lex(13),
                format!("invalid digit `{digit}` in {base} literal"),
                Primary::span(span(range)),
            ),
            MissingDigits(prefix, range) => Diagnostic::new(
                lex(13),
                format!("missing digits after `{prefix}`"),
                Primary::span(span(range)),
            ),
            NumberSuffix(rule, suffix, range) => Diagnostic::new(
                lex(*rule),
                format!("number literal with a suffix `{suffix}`"),
                Primary::span(span(range)),
            )
            .with_help(format!(
                "remove `{suffix}`: a literal takes the expected type"
            )),
            AfterExponent(rest, range) => Diagnostic::new(
                lex(14),
                format!("`{rest}` after the exponent"),
                Primary::span(span(range)),
            )
            .with_help(format!("remove `{rest}`: the exponent ends the literal")),
            FloatNotDecimal(range) => Diagnostic::new(
                lex(14),
                "float literal not in decimal",
                Primary::span(span(range)),
            ),
            UnterminatedCharacterLiteral(range) => Diagnostic::new(
                lex(15),
                "unterminated character literal",
                Primary::span(span(range)),
            ),
            EmptyCharacterLiteral(range) => Diagnostic::new(
                lex(15),
                "empty character literal",
                Primary::span(span(range)),
            ),
            MoreThanOneCharacter(content, range) => Diagnostic::new(
                lex(15),
                "character literal with more than one character",
                Primary::span(span(range)),
            )
            .with_help(format!("for a text, write `\"{content}\"`")),
            UnknownEscape(escape, range) => Diagnostic::new(
                lex(16),
                format!("unknown escape `{escape}`"),
                Primary::span(span(range)),
            )
            .with_help(r#"the escapes are `\n` `\r` `\t` `\0` `\\` `\'` `\"` `\{` `\}` `\u{…}`"#),
            MalformedUnicodeEscape(range) => Diagnostic::new(
                lex(16),
                "malformed Unicode escape",
                Primary::span(span(range)),
            )
            .with_help(r"write `\u{` with one to six hexadecimal digits, then `}`"),
            NotScalarValue(escape, range) => Diagnostic::new(
                lex(16),
                format!("`{escape}` is not a Unicode scalar value"),
                Primary::span(span(range)),
            ),
            UnterminatedRawText(range) => {
                Diagnostic::new(lex(19), "unterminated raw text", Primary::span(span(range)))
                    .with_help(r#"a raw text fits on one line and contains no `"`"#)
            }
            UnterminatedText(range) => {
                Diagnostic::new(lex(17), "unterminated text", Primary::span(span(range)))
                    .with_help(r#"a text fits on one line; for several lines, write `"""`"#)
            }
            UnmatchedBraceInText(range) => Diagnostic::new(
                lex(17),
                "unmatched `}` in a text",
                Primary::span(span(range)),
            )
            .with_help(r"for a literal brace, write `\}`"),
            EmptyInterpolation(range) => {
                Diagnostic::new(lex(20), "empty interpolation", Primary::span(span(range)))
            }
            UnterminatedInterpolation(range) => Diagnostic::new(
                lex(20),
                "unterminated interpolation",
                Primary::span(span(range)),
            ),
        }
    }
}

/// Tokenizes `text`, alone in a source map, and compares the result in full.
///
/// `expected_tokens` holds one line of `dump_tokens` per token, without its `\n`.
/// `expected_diagnostics` holds the diagnostics in text order.
fn assert_lexed(text: &str, expected_tokens: &[&str], expected_diagnostics: &[ExpectedDiagnostic]) {
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
fn digit_starts_a_number_not_an_identifier() {
    assert_lexed(
        "1x",
        &[r#"1:1 Int(Some(0)) "1x""#, r#"1:3 Eof """#],
        &[NumberSuffix(13, "x", 1..2)],
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

#[test]
fn dot_belongs_to_a_number_only_before_a_digit() {
    assert_lexed(
        "1..10",
        &[
            r#"1:1 Int(Some(1)) "1""#,
            r#"1:2 DotDot "..""#,
            r#"1:4 Int(Some(10)) "10""#,
            r#"1:6 Eof """#,
        ],
        &[],
    );
    assert_lexed(
        "5.abs()",
        &[
            r#"1:1 Int(Some(5)) "5""#,
            r#"1:2 Dot ".""#,
            r#"1:3 Identifier "abs""#,
            r#"1:6 OpenParen "(""#,
            r#"1:7 CloseParen ")""#,
            r#"1:8 Eof """#,
        ],
        &[],
    );
    assert_lexed(
        "1.e5",
        &[
            r#"1:1 Int(Some(1)) "1""#,
            r#"1:2 Dot ".""#,
            r#"1:3 Identifier "e5""#,
            r#"1:5 Eof """#,
        ],
        &[],
    );
    assert_lexed(
        "1.2.3",
        &[
            r#"1:1 Float("1.2") "1.2""#,
            r#"1:4 Dot ".""#,
            r#"1:5 Int(Some(3)) "3""#,
            r#"1:6 Eof """#,
        ],
        &[],
    );
    // S-05: a float has digits on both sides of its point.
    assert_lexed(
        "5.",
        &[r#"1:1 Int(Some(5)) "5""#, r#"1:2 Dot ".""#, r#"1:3 Eof """#],
        &[],
    );
    assert_lexed(
        ".5",
        &[r#"1:1 Dot ".""#, r#"1:2 Int(Some(5)) "5""#, r#"1:3 Eof """#],
        &[],
    );
}

#[test]
fn sign_belongs_to_a_number_only_after_a_decimal_exponent() {
    assert_one_token("2.5E-3", r#"Float("2.5E-3")"#);
    assert_lexed(
        "0x1e+5",
        &[
            r#"1:1 Int(Some(30)) "0x1e""#,
            r#"1:5 Plus "+""#,
            r#"1:6 Int(Some(5)) "5""#,
            r#"1:7 Eof """#,
        ],
        &[],
    );
    // In binary, `e` is not a digit, but it does not start an exponent either.
    assert_lexed(
        "0b1e+1",
        &[
            r#"1:1 Int(Some(0)) "0b1e""#,
            r#"1:5 Plus "+""#,
            r#"1:6 Int(Some(1)) "1""#,
            r#"1:7 Eof """#,
        ],
        &[NumberSuffix(13, "e", 3..4)],
    );
    // An uppercase prefix splits the text as its fix would.
    assert_lexed(
        "0X1e+5",
        &[
            r#"1:1 Int(Some(0)) "0X1e""#,
            r#"1:5 Plus "+""#,
            r#"1:6 Int(Some(5)) "5""#,
            r#"1:7 Eof """#,
        ],
        &[UppercasePrefix("0X", "0x", 0..2)],
    );
    // The literal goes past each sign that follows an `e` and comes before a digit.
    assert_lexed(
        "1e+5e+5",
        &[r#"1:1 Float("0") "1e+5e+5""#, r#"1:8 Eof """#],
        &[AfterExponent("e+5", 4..7)],
    );
}

#[test]
fn number_literal_gives_its_value() {
    assert_one_token("7", "Int(Some(7))");
    assert_one_token("18446744073709551616", "Int(None)");
    assert_one_token("1_000.000_1", r#"Float("1000.0001")"#);
}

#[test]
fn integer_with_a_leading_zero_is_refused() {
    assert_lexed(
        "007",
        &[r#"1:1 Int(Some(0)) "007""#, r#"1:4 Eof """#],
        &[LeadingZero("write `7`, or `0o7` for an octal number", 0..3)],
    );
    assert_lexed(
        "00",
        &[r#"1:1 Int(Some(0)) "00""#, r#"1:3 Eof """#],
        &[LeadingZero("write `0`, or `0o0` for an octal number", 0..2)],
    );
    assert_lexed(
        "0_7",
        &[r#"1:1 Int(Some(0)) "0_7""#, r#"1:4 Eof """#],
        &[LeadingZero("write `7`, or `0o7` for an octal number", 0..3)],
    );
}

#[test]
fn leading_zero_help_suggests_only_valid_literals() {
    assert_lexed(
        "089",
        &[r#"1:1 Int(Some(0)) "089""#, r#"1:4 Eof """#],
        &[LeadingZero("write `89`", 0..3)],
    );
    assert_lexed(
        "007_",
        &[r#"1:1 Int(Some(0)) "007_""#, r#"1:5 Eof """#],
        &[LeadingZero("remove the leading zeros", 0..4)],
    );
    // The `__` would go with the zeros, and `7` would hide it.
    assert_lexed(
        "0__7",
        &[r#"1:1 Int(Some(0)) "0__7""#, r#"1:5 Eof """#],
        &[LeadingZero("remove the leading zeros", 0..4)],
    );
    // A suffix is another defect too.
    assert_lexed(
        "007i64",
        &[r#"1:1 Int(Some(0)) "007i64""#, r#"1:7 Eof """#],
        &[LeadingZero("remove the leading zeros", 0..6)],
    );
}

#[test]
fn underscore_must_be_between_two_digits() {
    assert_lexed(
        "1__0",
        &[r#"1:1 Int(Some(0)) "1__0""#, r#"1:5 Eof """#],
        &[MisplacedUnderscore(13, 1..2)],
    );
    assert_lexed(
        "1_",
        &[r#"1:1 Int(Some(0)) "1_""#, r#"1:3 Eof """#],
        &[MisplacedUnderscore(13, 1..2)],
    );
    // One digit only: not a leading zero.
    assert_lexed(
        "0_",
        &[r#"1:1 Int(Some(0)) "0_""#, r#"1:3 Eof """#],
        &[MisplacedUnderscore(13, 1..2)],
    );
    assert_lexed(
        "0x_1",
        &[r#"1:1 Int(Some(0)) "0x_1""#, r#"1:5 Eof """#],
        &[MisplacedUnderscore(13, 2..3)],
    );
    assert_lexed(
        "1_e5",
        &[r#"1:1 Float("0") "1_e5""#, r#"1:5 Eof """#],
        &[MisplacedUnderscore(14, 1..2)],
    );
}

#[test]
fn uppercase_prefix_is_refused_with_a_fix() {
    assert_lexed(
        "0X1F",
        &[r#"1:1 Int(Some(0)) "0X1F""#, r#"1:5 Eof """#],
        &[UppercasePrefix("0X", "0x", 0..2)],
    );
    assert_lexed(
        "0B1",
        &[r#"1:1 Int(Some(0)) "0B1""#, r#"1:4 Eof """#],
        &[UppercasePrefix("0B", "0b", 0..2)],
    );
    assert_lexed(
        "0O7",
        &[r#"1:1 Int(Some(0)) "0O7""#, r#"1:4 Eof """#],
        &[UppercasePrefix("0O", "0o", 0..2)],
    );
}

#[test]
fn digit_outside_the_base_is_refused() {
    assert_lexed(
        "0b102",
        &[r#"1:1 Int(Some(0)) "0b102""#, r#"1:6 Eof """#],
        &[InvalidDigit('2', "a binary", 4..5)],
    );
    assert_lexed(
        "0o8",
        &[r#"1:1 Int(Some(0)) "0o8""#, r#"1:4 Eof """#],
        &[InvalidDigit('8', "an octal", 2..3)],
    );
    assert_lexed(
        "0xG",
        &[r#"1:1 Int(Some(0)) "0xG""#, r#"1:4 Eof """#],
        &[InvalidDigit('G', "a hexadecimal", 2..3)],
    );
}

#[test]
fn prefix_without_digits_is_refused() {
    assert_lexed(
        "0x",
        &[r#"1:1 Int(Some(0)) "0x""#, r#"1:3 Eof """#],
        &[MissingDigits("0x", 0..2)],
    );
}

#[test]
fn number_suffix_is_refused() {
    assert_lexed(
        "10i64",
        &[r#"1:1 Int(Some(0)) "10i64""#, r#"1:6 Eof """#],
        &[NumberSuffix(13, "i64", 2..5)],
    );
    assert_lexed(
        "1e",
        &[r#"1:1 Int(Some(0)) "1e""#, r#"1:3 Eof """#],
        &[NumberSuffix(13, "e", 1..2)],
    );
    assert_lexed(
        "1.5f32",
        &[r#"1:1 Float("0") "1.5f32""#, r#"1:7 Eof """#],
        &[NumberSuffix(14, "f32", 3..6)],
    );
    assert_lexed(
        "0x1z",
        &[r#"1:1 Int(Some(0)) "0x1z""#, r#"1:5 Eof """#],
        &[NumberSuffix(13, "z", 3..4)],
    );
}

#[test]
fn point_after_the_exponent_is_refused() {
    assert_lexed(
        "1e5.5",
        &[r#"1:1 Float("0") "1e5.5""#, r#"1:6 Eof """#],
        &[AfterExponent(".5", 3..5)],
    );
}

#[test]
fn float_not_in_decimal_is_refused() {
    assert_lexed(
        "0x1.5",
        &[r#"1:1 Float("0") "0x1.5""#, r#"1:6 Eof """#],
        &[FloatNotDecimal(0..5)],
    );
}

#[test]
fn incomplete_exponent_ends_the_literal() {
    assert_lexed(
        "1e+x",
        &[
            r#"1:1 Int(Some(0)) "1e""#,
            r#"1:3 Plus "+""#,
            r#"1:4 Identifier "x""#,
            r#"1:5 Eof """#,
        ],
        &[NumberSuffix(13, "e", 1..2)],
    );
}

#[test]
fn number_literal_gets_one_diagnostic() {
    assert_lexed(
        "0X1__0",
        &[r#"1:1 Int(Some(0)) "0X1__0""#, r#"1:7 Eof """#],
        &[UppercasePrefix("0X", "0x", 0..2)],
    );
}

#[test]
fn lexing_goes_on_after_a_refused_literal() {
    assert_lexed(
        "007 + 1",
        &[
            r#"1:1 Int(Some(0)) "007""#,
            r#"1:5 Plus "+""#,
            r#"1:7 Int(Some(1)) "1""#,
            r#"1:8 Eof """#,
        ],
        &[LeadingZero("write `7`, or `0o7` for an octal number", 0..3)],
    );
}

#[test]
fn literal_diagnostic_is_placed_in_the_text() {
    assert_lexed(
        "1 + 0X1F",
        &[
            r#"1:1 Int(Some(1)) "1""#,
            r#"1:3 Plus "+""#,
            r#"1:5 Int(Some(0)) "0X1F""#,
            r#"1:9 Eof """#,
        ],
        &[UppercasePrefix("0X", "0x", 4..6)],
    );
}

#[test]
fn number_literal_reads_only_ascii() {
    // `is_numeric` and `is_alphanumeric` would take these characters (C.32).
    assert_lexed("٣", &[r#"1:2 Eof """#], &[UnexpectedCharacter('٣', 0..2)]);
    assert_lexed(
        "1²",
        &[r#"1:1 Int(Some(1)) "1""#, r#"1:3 Eof """#],
        &[UnexpectedCharacter('²', 1..3)],
    );
    assert_lexed(
        "1.٣",
        &[r#"1:1 Int(Some(1)) "1""#, r#"1:2 Dot ".""#, r#"1:4 Eof """#],
        &[UnexpectedCharacter('٣', 2..4)],
    );
    assert_lexed(
        "1e+٣",
        &[
            r#"1:1 Int(Some(0)) "1e""#,
            r#"1:3 Plus "+""#,
            r#"1:5 Eof """#,
        ],
        &[NumberSuffix(13, "e", 1..2), UnexpectedCharacter('٣', 3..5)],
    );
}

#[test]
fn character_literal_gives_its_value() {
    assert_one_token("'a'", "Char('a')");
    assert_one_token(r#"'"'"#, r#"Char('"')"#);
    // A tab written as it is, not as `\t`.
    assert_one_token("'\t'", r"Char('\t')");
    assert_lexed("'é'", &[r#"1:1 Char('é') "'é'""#, r#"1:4 Eof """#], &[]);
    assert_lexed("'😀'", &[r#"1:1 Char('😀') "'😀'""#, r#"1:4 Eof """#], &[]);
}

#[test]
fn escape_gives_its_character() {
    assert_one_token(r"'\n'", r"Char('\n')");
    assert_one_token(r"'\r'", r"Char('\r')");
    assert_one_token(r"'\t'", r"Char('\t')");
    assert_one_token(r"'\0'", r"Char('\0')");
    assert_one_token(r"'\\'", r"Char('\\')");
    assert_one_token(r"'\''", r"Char('\'')");
    assert_one_token(r#"'\"'"#, r#"Char('"')"#);
    assert_one_token(r"'\{'", "Char('{')");
    assert_one_token(r"'\}'", "Char('}')");
}

#[test]
fn unicode_escape_gives_its_value() {
    assert_one_token(r"'\u{41}'", "Char('A')");
    assert_one_token(r"'\u{e9}'", "Char('é')");
    assert_one_token(r"'\u{0}'", r"Char('\0')");
    assert_one_token(r"'\u{1F600}'", "Char('😀')");
    assert_one_token(r"'\u{10FFFF}'", r"Char('\u{10ffff}')");
}

#[test]
fn empty_character_literal_is_refused() {
    assert_lexed(
        "''",
        &[r#"1:1 Char('�') "''""#, r#"1:3 Eof """#],
        &[EmptyCharacterLiteral(0..2)],
    );
}

#[test]
fn character_literal_with_more_than_one_value_is_refused() {
    assert_lexed(
        "'ab'",
        &[r#"1:1 Char('�') "'ab'""#, r#"1:5 Eof """#],
        &[MoreThanOneCharacter("ab", 0..4)],
    );
    // `é` written as `e`, then the combining accent U+0301: two scalar values.
    assert_lexed(
        "'e\u{301}'",
        &[r#"1:1 Char('�') "'e\u{301}'""#, r#"1:5 Eof """#],
        &[MoreThanOneCharacter("e\u{301}", 0..5)],
    );
    // An escape is one value, wherever it stands.
    assert_lexed(
        r"'\na'",
        &[r#"1:1 Char('�') "'\\na'""#, r#"1:6 Eof """#],
        &[MoreThanOneCharacter(r"\na", 0..5)],
    );
    assert_lexed(
        r"'a\n'",
        &[r#"1:1 Char('�') "'a\\n'""#, r#"1:6 Eof """#],
        &[MoreThanOneCharacter(r"a\n", 0..5)],
    );
}

#[test]
fn unterminated_character_literal_runs_to_the_end_of_the_line() {
    assert_lexed(
        "'a",
        &[r#"1:1 Char('�') "'a""#, r#"1:3 Eof """#],
        &[UnterminatedCharacterLiteral(0..2)],
    );
    assert_lexed(
        "'",
        &[r#"1:1 Char('�') "'""#, r#"1:2 Eof """#],
        &[UnterminatedCharacterLiteral(0..1)],
    );
    assert_lexed(
        "'a\r\n",
        &[
            r#"1:1 Char('�') "'a""#,
            r#"1:3 Newline "\r\n""#,
            r#"2:1 Eof """#,
        ],
        &[UnterminatedCharacterLiteral(0..2)],
    );
    // The `'` of the next line opens another literal.
    assert_lexed(
        "'a\nb'",
        &[
            r#"1:1 Char('�') "'a""#,
            r#"1:3 Newline "\n""#,
            r#"2:1 Identifier "b""#,
            r#"2:2 Char('�') "'""#,
            r#"2:3 Eof """#,
        ],
        &[
            UnterminatedCharacterLiteral(0..2),
            UnterminatedCharacterLiteral(4..5),
        ],
    );
}

#[test]
fn escaped_quote_does_not_end_the_literal() {
    assert_lexed(
        r"'\'",
        &[r#"1:1 Char('�') "'\\'""#, r#"1:4 Eof """#],
        &[UnterminatedCharacterLiteral(0..3)],
    );
}

#[test]
fn backslash_does_not_take_the_line_ending() {
    assert_lexed(
        "'\\\n",
        &[
            r#"1:1 Char('�') "'\\""#,
            r#"1:3 Newline "\n""#,
            r#"2:1 Eof """#,
        ],
        &[UnterminatedCharacterLiteral(0..2)],
    );
}

#[test]
fn unknown_escape_is_refused() {
    assert_lexed(
        r"'\q'",
        &[r#"1:1 Char('�') "'\\q'""#, r#"1:5 Eof """#],
        &[UnknownEscape(r"\q", 1..3)],
    );
}

#[test]
fn malformed_unicode_escape_is_refused() {
    // Without `{`, the span stops after `\u`.
    assert_lexed(
        r"'\u41'",
        &[r#"1:1 Char('�') "'\\u41'""#, r#"1:7 Eof """#],
        &[MalformedUnicodeEscape(1..3)],
    );
    // With `}` after the digits, the span covers the whole escape.
    assert_lexed(
        r"'\u{}'",
        &[r#"1:1 Char('�') "'\\u{}'""#, r#"1:7 Eof """#],
        &[MalformedUnicodeEscape(1..5)],
    );
    assert_lexed(
        r"'\u{1234567}'",
        &[r#"1:1 Char('�') "'\\u{1234567}'""#, r#"1:14 Eof """#],
        &[MalformedUnicodeEscape(1..12)],
    );
    // Without `}` after the digits, the span stops after them.
    assert_lexed(
        r"'\u{4G}'",
        &[r#"1:1 Char('�') "'\\u{4G}'""#, r#"1:9 Eof """#],
        &[MalformedUnicodeEscape(1..5)],
    );
    assert_lexed(
        r"'\u{41'",
        &[r#"1:1 Char('�') "'\\u{41'""#, r#"1:8 Eof """#],
        &[MalformedUnicodeEscape(1..6)],
    );
}

#[test]
fn unicode_escape_outside_scalar_values_is_refused() {
    assert_lexed(
        r"'\u{D800}'",
        &[r#"1:1 Char('�') "'\\u{D800}'""#, r#"1:11 Eof """#],
        &[NotScalarValue(r"\u{D800}", 1..9)],
    );
    assert_lexed(
        r"'\u{110000}'",
        &[r#"1:1 Char('�') "'\\u{110000}'""#, r#"1:13 Eof """#],
        &[NotScalarValue(r"\u{110000}", 1..11)],
    );
}

#[test]
fn character_literal_gets_one_diagnostic() {
    // The refused escape is reported, not the second value.
    assert_lexed(
        r"'\qa'",
        &[r#"1:1 Char('�') "'\\qa'""#, r#"1:6 Eof """#],
        &[UnknownEscape(r"\q", 1..3)],
    );
    // The second value is not read, so its escape is not checked.
    assert_lexed(
        r"'a\q'",
        &[r#"1:1 Char('�') "'a\\q'""#, r#"1:6 Eof """#],
        &[MoreThanOneCharacter(r"a\q", 0..5)],
    );
    // Without its closing `'`, the literal's content is not read.
    assert_lexed(
        r"'\q",
        &[r#"1:1 Char('�') "'\\q""#, r#"1:4 Eof """#],
        &[UnterminatedCharacterLiteral(0..3)],
    );
}

#[test]
fn lexing_goes_on_after_a_refused_character_literal() {
    assert_lexed(
        "'ab' + 'c'",
        &[
            r#"1:1 Char('�') "'ab'""#,
            r#"1:6 Plus "+""#,
            r#"1:8 Char('c') "'c'""#,
            r#"1:11 Eof """#,
        ],
        &[MoreThanOneCharacter("ab", 0..4)],
    );
}

#[test]
fn escape_diagnostic_is_placed_in_the_text() {
    assert_lexed(
        r"x = '\q'",
        &[
            r#"1:1 Identifier "x""#,
            r#"1:3 Eq "=""#,
            r#"1:5 Char('�') "'\\q'""#,
            r#"1:9 Eof """#,
        ],
        &[UnknownEscape(r"\q", 5..7)],
    );
}

#[test]
fn help_escapes_what_a_text_would_read() {
    // In a text, `"` would end it, and `{x}` would read `x`.
    assert_lexed(
        r#"'a"'"#,
        &[r#"1:1 Char('�') "'a\"'""#, r#"1:5 Eof """#],
        &[MoreThanOneCharacter(r#"a\""#, 0..4)],
    );
    assert_lexed(
        "'{x}'",
        &[r#"1:1 Char('�') "'{x}'""#, r#"1:6 Eof """#],
        &[MoreThanOneCharacter(r"\{x\}", 0..5)],
    );
    // An escape stays as written, its braces too.
    assert_lexed(
        r#"'\"a'"#,
        &[r#"1:1 Char('�') "'\\\"a'""#, r#"1:6 Eof """#],
        &[MoreThanOneCharacter(r#"\"a"#, 0..5)],
    );
    assert_lexed(
        r#"'\\"'"#,
        &[r#"1:1 Char('�') "'\\\\\"'""#, r#"1:6 Eof """#],
        &[MoreThanOneCharacter(r#"\\\""#, 0..5)],
    );
    assert_lexed(
        r"'\u{41}}'",
        &[r#"1:1 Char('�') "'\\u{41}}'""#, r#"1:10 Eof """#],
        &[MoreThanOneCharacter(r"\u{41}\}", 0..9)],
    );
    // A refused escape stays as the reader delimits it: here `\u{4`.
    assert_lexed(
        r"'a\u{4G}'",
        &[r#"1:1 Char('�') "'a\\u{4G}'""#, r#"1:10 Eof """#],
        &[MoreThanOneCharacter(r"a\u{4G\}", 0..9)],
    );
}

#[test]
fn character_literal_opens_no_delimiter() {
    // Inside `(`, the line ending would give no token.
    assert_lexed(
        "'('\nx",
        &[
            r#"1:1 Char('(') "'('""#,
            r#"1:4 Newline "\n""#,
            r#"2:1 Identifier "x""#,
            r#"2:2 Eof """#,
        ],
        &[],
    );
}

#[test]
fn raw_text_keeps_each_character_as_written() {
    // Neither `\` nor `{` means anything here: these are 14 characters.
    assert_one_token(r#"r"C:\Users\{nom}""#, r#"RawText("C:\\Users\\{nom}")"#);
    assert_one_token(r#"r"\n""#, r#"RawText("\\n")"#);
    // `\"` escapes nothing: the quote ends the text.
    assert_one_token(r#"r"a\""#, r#"RawText("a\\")"#);
}

#[test]
fn empty_raw_text_gives_an_empty_content() {
    assert_lexed(
        r#"r"""#,
        &[r#"1:1 RawText("") "r\"\"""#, r#"1:4 Eof """#],
        &[],
    );
}

#[test]
fn raw_text_is_not_a_comment() {
    assert_one_token(
        r#"r"// pas un commentaire""#,
        r#"RawText("// pas un commentaire")"#,
    );
}

#[test]
fn unterminated_raw_text_runs_to_the_end_of_the_line() {
    assert_lexed(
        r#"r"abc"#,
        &[r#"1:1 RawText("abc") "r\"abc""#, r#"1:6 Eof """#],
        &[UnterminatedRawText(0..5)],
    );
    assert_lexed(
        r#"r""#,
        &[r#"1:1 RawText("") "r\"""#, r#"1:3 Eof """#],
        &[UnterminatedRawText(0..2)],
    );
    // The span counts bytes, and `é` takes two.
    assert_lexed(
        r#"r"é"#,
        &[r#"1:1 RawText("é") "r\"é""#, r#"1:4 Eof """#],
        &[UnterminatedRawText(0..4)],
    );
    // Lexing goes on at the line ending.
    assert_lexed(
        "r\"abc\n+",
        &[
            r#"1:1 RawText("abc") "r\"abc""#,
            r#"1:6 Newline "\n""#,
            r#"2:1 Plus "+""#,
            r#"2:2 Eof """#,
        ],
        &[UnterminatedRawText(0..5)],
    );
    assert_lexed(
        "r\"abc\r\n+",
        &[
            r#"1:1 RawText("abc") "r\"abc""#,
            r#"1:6 Newline "\r\n""#,
            r#"2:1 Plus "+""#,
            r#"2:2 Eof """#,
        ],
        &[UnterminatedRawText(0..5)],
    );
}

#[test]
fn r_alone_is_an_identifier() {
    assert_lexed(
        "r + r2",
        &[
            r#"1:1 Identifier "r""#,
            r#"1:3 Plus "+""#,
            r#"1:5 Identifier "r2""#,
            r#"1:7 Eof """#,
        ],
        &[],
    );
}

#[test]
fn raw_text_is_placed_in_the_text() {
    assert_lexed(
        r#"x = r"é" + 1"#,
        &[
            r#"1:1 Identifier "x""#,
            r#"1:3 Eq "=""#,
            r#"1:5 RawText("é") "r\"é\"""#,
            r#"1:10 Plus "+""#,
            r#"1:12 Int(Some(1)) "1""#,
            r#"1:13 Eof """#,
        ],
        &[],
    );
}

#[test]
fn raw_text_opens_no_delimiter() {
    // Inside `(`, the line ending would give no token.
    assert_lexed(
        "r\"(\"\nx",
        &[
            r#"1:1 RawText("(") "r\"(\"""#,
            r#"1:5 Newline "\n""#,
            r#"2:1 Identifier "x""#,
            r#"2:2 Eof """#,
        ],
        &[],
    );
}

#[test]
fn text_gives_its_start_its_piece_and_its_end() {
    assert_lexed(
        r#""abc""#,
        &[
            r#"1:1 TextStart "\"""#,
            r#"1:2 TextPiece("abc") "abc""#,
            r#"1:5 TextEnd "\"""#,
            r#"1:6 Eof """#,
        ],
        &[],
    );
    // No empty piece.
    assert_lexed(
        r#""""#,
        &[
            r#"1:1 TextStart "\"""#,
            r#"1:2 TextEnd "\"""#,
            r#"1:3 Eof """#,
        ],
        &[],
    );
}

#[test]
fn interpolation_and_its_format_give_their_tokens() {
    assert_lexed(
        r#""a\t{b:>8}""#,
        &[
            r#"1:1 TextStart "\"""#,
            r#"1:2 TextPiece("a\t") "a\\t""#,
            r#"1:5 InterpolationStart "{""#,
            r#"1:6 Identifier "b""#,
            r#"1:7 Format(">8") ":>8""#,
            r#"1:10 InterpolationEnd "}""#,
            r#"1:11 TextEnd "\"""#,
            r#"1:12 Eof """#,
        ],
        &[],
    );
}

#[test]
fn escapes_give_their_characters_in_one_piece() {
    assert_lexed(
        r#""a\tb\u{e9}\{x\}""#,
        &[
            r#"1:1 TextStart "\"""#,
            r#"1:2 TextPiece("a\tbé{x}") "a\\tb\\u{e9}\\{x\\}""#,
            r#"1:17 TextEnd "\"""#,
            r#"1:18 Eof """#,
        ],
        &[],
    );
}

#[test]
fn refused_escape_in_a_text_gives_u_fffd() {
    // The text goes on after it.
    assert_lexed(
        r#""a\qb""#,
        &[
            r#"1:1 TextStart "\"""#,
            r#"1:2 TextPiece("a�b") "a\\qb""#,
            r#"1:6 TextEnd "\"""#,
            r#"1:7 Eof """#,
        ],
        &[UnknownEscape(r"\q", 2..4)],
    );
}

#[test]
fn text_is_not_a_comment() {
    assert_lexed(
        r#""a // b""#,
        &[
            r#"1:1 TextStart "\"""#,
            r#"1:2 TextPiece("a // b") "a // b""#,
            r#"1:8 TextEnd "\"""#,
            r#"1:9 Eof """#,
        ],
        &[],
    );
}

#[test]
fn examples_of_lex_20_give_their_tokens() {
    let cases: [(&str, &[&str]); 5] = [
        (
            r#""{name}""#,
            &[
                r#"1:1 TextStart "\"""#,
                r#"1:2 InterpolationStart "{""#,
                r#"1:3 Identifier "name""#,
                r#"1:7 InterpolationEnd "}""#,
                r#"1:8 TextEnd "\"""#,
                r#"1:9 Eof """#,
            ],
        ),
        (
            r#""{removed.len()} fichier(s)""#,
            &[
                r#"1:1 TextStart "\"""#,
                r#"1:2 InterpolationStart "{""#,
                r#"1:3 Identifier "removed""#,
                r#"1:10 Dot ".""#,
                r#"1:11 Identifier "len""#,
                r#"1:14 OpenParen "(""#,
                r#"1:15 CloseParen ")""#,
                r#"1:16 InterpolationEnd "}""#,
                r#"1:17 TextPiece(" fichier(s)") " fichier(s)""#,
                r#"1:28 TextEnd "\"""#,
                r#"1:29 Eof """#,
            ],
        ),
        (
            r#""{cents / 100:>8}""#,
            &[
                r#"1:1 TextStart "\"""#,
                r#"1:2 InterpolationStart "{""#,
                r#"1:3 Identifier "cents""#,
                r#"1:9 Slash "/""#,
                r#"1:11 Int(Some(100)) "100""#,
                r#"1:14 Format(">8") ":>8""#,
                r#"1:17 InterpolationEnd "}""#,
                r#"1:18 TextEnd "\"""#,
                r#"1:19 Eof """#,
            ],
        ),
        (
            r#""{names.join(", ")}""#,
            &[
                r#"1:1 TextStart "\"""#,
                r#"1:2 InterpolationStart "{""#,
                r#"1:3 Identifier "names""#,
                r#"1:8 Dot ".""#,
                r#"1:9 Identifier "join""#,
                r#"1:13 OpenParen "(""#,
                r#"1:14 TextStart "\"""#,
                r#"1:15 TextPiece(", ") ", ""#,
                r#"1:17 TextEnd "\"""#,
                r#"1:18 CloseParen ")""#,
                r#"1:19 InterpolationEnd "}""#,
                r#"1:20 TextEnd "\"""#,
                r#"1:21 Eof """#,
            ],
        ),
        (
            r#""\{name\}""#,
            &[
                r#"1:1 TextStart "\"""#,
                r#"1:2 TextPiece("{name}") "\\{name\\}""#,
                r#"1:10 TextEnd "\"""#,
                r#"1:11 Eof """#,
            ],
        ),
    ];

    for (text, expected_tokens) in cases {
        assert_lexed(text, expected_tokens, &[]);
    }
}

#[test]
fn colon_opens_the_format_only_at_depth_zero() {
    let cases: [(&str, &[&str]); 3] = [
        (
            r#""{f(a: 1):x}""#,
            &[
                r#"1:1 TextStart "\"""#,
                r#"1:2 InterpolationStart "{""#,
                r#"1:3 Identifier "f""#,
                r#"1:4 OpenParen "(""#,
                r#"1:5 Identifier "a""#,
                r#"1:6 Colon ":""#,
                r#"1:8 Int(Some(1)) "1""#,
                r#"1:9 CloseParen ")""#,
                r#"1:10 Format("x") ":x""#,
                r#"1:12 InterpolationEnd "}""#,
                r#"1:13 TextEnd "\"""#,
                r#"1:14 Eof """#,
            ],
        ),
        (
            r#""{xs[a:b]}""#,
            &[
                r#"1:1 TextStart "\"""#,
                r#"1:2 InterpolationStart "{""#,
                r#"1:3 Identifier "xs""#,
                r#"1:5 OpenBracket "[""#,
                r#"1:6 Identifier "a""#,
                r#"1:7 Colon ":""#,
                r#"1:8 Identifier "b""#,
                r#"1:9 CloseBracket "]""#,
                r#"1:10 InterpolationEnd "}""#,
                r#"1:11 TextEnd "\"""#,
                r#"1:12 Eof """#,
            ],
        ),
        (
            r#""{{a: 1}}""#,
            &[
                r#"1:1 TextStart "\"""#,
                r#"1:2 InterpolationStart "{""#,
                r#"1:3 OpenBrace "{""#,
                r#"1:4 Identifier "a""#,
                r#"1:5 Colon ":""#,
                r#"1:7 Int(Some(1)) "1""#,
                r#"1:8 CloseBrace "}""#,
                r#"1:9 InterpolationEnd "}""#,
                r#"1:10 TextEnd "\"""#,
                r#"1:11 Eof """#,
            ],
        ),
    ];

    for (text, expected_tokens) in cases {
        assert_lexed(text, expected_tokens, &[]);
    }
}

#[test]
fn braces_of_a_block_in_an_interpolation_are_delimiters() {
    assert_lexed(
        r#""{if a { 1 } else { 2 }}""#,
        &[
            r#"1:1 TextStart "\"""#,
            r#"1:2 InterpolationStart "{""#,
            r#"1:3 Keyword(If) "if""#,
            r#"1:6 Identifier "a""#,
            r#"1:8 OpenBrace "{""#,
            r#"1:10 Int(Some(1)) "1""#,
            r#"1:12 CloseBrace "}""#,
            r#"1:14 Keyword(Else) "else""#,
            r#"1:19 OpenBrace "{""#,
            r#"1:21 Int(Some(2)) "2""#,
            r#"1:23 CloseBrace "}""#,
            r#"1:24 InterpolationEnd "}""#,
            r#"1:25 TextEnd "\"""#,
            r#"1:26 Eof """#,
        ],
        &[],
    );
}

#[test]
fn text_inside_an_interpolation_follows_the_same_rules() {
    // The example of the lexing notion sheet, §4.
    assert_lexed(
        r#""a{f("b{x}")}c""#,
        &[
            r#"1:1 TextStart "\"""#,
            r#"1:2 TextPiece("a") "a""#,
            r#"1:3 InterpolationStart "{""#,
            r#"1:4 Identifier "f""#,
            r#"1:5 OpenParen "(""#,
            r#"1:6 TextStart "\"""#,
            r#"1:7 TextPiece("b") "b""#,
            r#"1:8 InterpolationStart "{""#,
            r#"1:9 Identifier "x""#,
            r#"1:10 InterpolationEnd "}""#,
            r#"1:11 TextEnd "\"""#,
            r#"1:12 CloseParen ")""#,
            r#"1:13 InterpolationEnd "}""#,
            r#"1:14 TextPiece("c") "c""#,
            r#"1:15 TextEnd "\"""#,
            r#"1:16 Eof """#,
        ],
        &[],
    );
}

#[test]
fn neighboring_interpolations_give_no_empty_piece() {
    assert_lexed(
        r#""{a}{b}""#,
        &[
            r#"1:1 TextStart "\"""#,
            r#"1:2 InterpolationStart "{""#,
            r#"1:3 Identifier "a""#,
            r#"1:4 InterpolationEnd "}""#,
            r#"1:5 InterpolationStart "{""#,
            r#"1:6 Identifier "b""#,
            r#"1:7 InterpolationEnd "}""#,
            r#"1:8 TextEnd "\"""#,
            r#"1:9 Eof """#,
        ],
        &[],
    );
}

#[test]
fn two_slashes_in_an_interpolation_are_two_slashes() {
    assert_lexed(
        r#""{a // b}""#,
        &[
            r#"1:1 TextStart "\"""#,
            r#"1:2 InterpolationStart "{""#,
            r#"1:3 Identifier "a""#,
            r#"1:5 Slash "/""#,
            r#"1:6 Slash "/""#,
            r#"1:8 Identifier "b""#,
            r#"1:9 InterpolationEnd "}""#,
            r#"1:10 TextEnd "\"""#,
            r#"1:11 Eof """#,
        ],
        &[],
    );
}

#[test]
fn two_slashes_after_a_text_open_a_comment() {
    assert_lexed(
        r#""{a}" // b"#,
        &[
            r#"1:1 TextStart "\"""#,
            r#"1:2 InterpolationStart "{""#,
            r#"1:3 Identifier "a""#,
            r#"1:4 InterpolationEnd "}""#,
            r#"1:5 TextEnd "\"""#,
            r#"1:11 Eof """#,
        ],
        &[],
    );
}

#[test]
fn empty_format_gives_an_empty_value() {
    assert_lexed(
        r#""{x:}""#,
        &[
            r#"1:1 TextStart "\"""#,
            r#"1:2 InterpolationStart "{""#,
            r#"1:3 Identifier "x""#,
            r#"1:4 Format("") ":""#,
            r#"1:5 InterpolationEnd "}""#,
            r#"1:6 TextEnd "\"""#,
            r#"1:7 Eof """#,
        ],
        &[],
    );
}

#[test]
fn unterminated_text_runs_to_the_end_of_the_line() {
    assert_lexed(
        r#""abc"#,
        &[
            r#"1:1 TextStart "\"""#,
            r#"1:2 TextPiece("abc") "abc""#,
            r#"1:5 TextEnd """#,
            r#"1:5 Eof """#,
        ],
        &[UnterminatedText(0..4)],
    );
    assert_lexed(
        r#"""#,
        &[
            r#"1:1 TextStart "\"""#,
            r#"1:2 TextEnd """#,
            r#"1:2 Eof """#,
        ],
        &[UnterminatedText(0..1)],
    );
    // The end token comes before the line ending, which lexing reads as usual.
    assert_lexed(
        "\"abc\n+",
        &[
            r#"1:1 TextStart "\"""#,
            r#"1:2 TextPiece("abc") "abc""#,
            r#"1:5 TextEnd """#,
            r#"1:5 Newline "\n""#,
            r#"2:1 Plus "+""#,
            r#"2:2 Eof """#,
        ],
        &[UnterminatedText(0..4)],
    );
    assert_lexed(
        "\"abc\r\n+",
        &[
            r#"1:1 TextStart "\"""#,
            r#"1:2 TextPiece("abc") "abc""#,
            r#"1:5 TextEnd """#,
            r#"1:5 Newline "\r\n""#,
            r#"2:1 Plus "+""#,
            r#"2:2 Eof """#,
        ],
        &[UnterminatedText(0..4)],
    );
    // After an interpolation, the diagnostic still starts at the text's `"`.
    assert_lexed(
        r#""{a}b"#,
        &[
            r#"1:1 TextStart "\"""#,
            r#"1:2 InterpolationStart "{""#,
            r#"1:3 Identifier "a""#,
            r#"1:4 InterpolationEnd "}""#,
            r#"1:5 TextPiece("b") "b""#,
            r#"1:6 TextEnd """#,
            r#"1:6 Eof """#,
        ],
        &[UnterminatedText(0..5)],
    );
}

#[test]
fn lone_closing_brace_is_refused_and_kept_in_the_piece() {
    assert_lexed(
        r#""a}b""#,
        &[
            r#"1:1 TextStart "\"""#,
            r#"1:2 TextPiece("a}b") "a}b""#,
            r#"1:5 TextEnd "\"""#,
            r#"1:6 Eof """#,
        ],
        &[UnmatchedBraceInText(2..3)],
    );
}

#[test]
fn empty_interpolation_is_refused() {
    assert_lexed(
        r#""{}""#,
        &[
            r#"1:1 TextStart "\"""#,
            r#"1:2 InterpolationStart "{""#,
            r#"1:3 InterpolationEnd "}""#,
            r#"1:4 TextEnd "\"""#,
            r#"1:5 Eof """#,
        ],
        &[EmptyInterpolation(1..3)],
    );
    for text in [r#""{ }""#, "\"{\t}\""] {
        assert_lexed(
            text,
            &[
                r#"1:1 TextStart "\"""#,
                r#"1:2 InterpolationStart "{""#,
                r#"1:4 InterpolationEnd "}""#,
                r#"1:5 TextEnd "\"""#,
                r#"1:6 Eof """#,
            ],
            &[EmptyInterpolation(1..4)],
        );
    }
}

#[test]
fn only_spaces_and_tabs_make_an_interpolation_empty() {
    // A no-break space is an unexpected character.
    assert_lexed(
        "\"{\u{a0}}\"",
        &[
            r#"1:1 TextStart "\"""#,
            r#"1:2 InterpolationStart "{""#,
            r#"1:4 InterpolationEnd "}""#,
            r#"1:5 TextEnd "\"""#,
            r#"1:6 Eof """#,
        ],
        &[UnexpectedCharacter('\u{a0}', 2..4)],
    );
    // A format alone is not empty: checking the expression is the parser's job.
    assert_lexed(
        r#""{:x}""#,
        &[
            r#"1:1 TextStart "\"""#,
            r#"1:2 InterpolationStart "{""#,
            r#"1:3 Format("x") ":x""#,
            r#"1:5 InterpolationEnd "}""#,
            r#"1:6 TextEnd "\"""#,
            r#"1:7 Eof """#,
        ],
        &[],
    );
}

#[test]
fn unterminated_interpolation_ends_with_its_text_at_the_line_end() {
    assert_lexed(
        r#""{a"#,
        &[
            r#"1:1 TextStart "\"""#,
            r#"1:2 InterpolationStart "{""#,
            r#"1:3 Identifier "a""#,
            r#"1:4 InterpolationEnd """#,
            r#"1:4 TextEnd """#,
            r#"1:4 Eof """#,
        ],
        &[UnterminatedInterpolation(1..3)],
    );
    assert_lexed(
        "\"{a\n+",
        &[
            r#"1:1 TextStart "\"""#,
            r#"1:2 InterpolationStart "{""#,
            r#"1:3 Identifier "a""#,
            r#"1:4 InterpolationEnd """#,
            r#"1:4 TextEnd """#,
            r#"1:4 Newline "\n""#,
            r#"2:1 Plus "+""#,
            r#"2:2 Eof """#,
        ],
        &[UnterminatedInterpolation(1..3)],
    );
    // Under a `(`, which closes with the interpolation: the line ending gives a token.
    assert_lexed(
        "\"{f(a\n+",
        &[
            r#"1:1 TextStart "\"""#,
            r#"1:2 InterpolationStart "{""#,
            r#"1:3 Identifier "f""#,
            r#"1:4 OpenParen "(""#,
            r#"1:5 Identifier "a""#,
            r#"1:6 InterpolationEnd """#,
            r#"1:6 TextEnd """#,
            r#"1:6 Newline "\n""#,
            r#"2:1 Plus "+""#,
            r#"2:2 Eof """#,
        ],
        &[UnterminatedInterpolation(1..5)],
    );
    // In a format, which stops at the line end.
    assert_lexed(
        "\"{x:ab\n+",
        &[
            r#"1:1 TextStart "\"""#,
            r#"1:2 InterpolationStart "{""#,
            r#"1:3 Identifier "x""#,
            r#"1:4 Format("ab") ":ab""#,
            r#"1:7 InterpolationEnd """#,
            r#"1:7 TextEnd """#,
            r#"1:7 Newline "\n""#,
            r#"2:1 Plus "+""#,
            r#"2:2 Eof """#,
        ],
        &[UnterminatedInterpolation(1..6)],
    );
}

#[test]
fn line_end_is_reported_once_by_the_innermost_cut() {
    assert_lexed(
        "\"{f(\"{a\n+",
        &[
            r#"1:1 TextStart "\"""#,
            r#"1:2 InterpolationStart "{""#,
            r#"1:3 Identifier "f""#,
            r#"1:4 OpenParen "(""#,
            r#"1:5 TextStart "\"""#,
            r#"1:6 InterpolationStart "{""#,
            r#"1:7 Identifier "a""#,
            r#"1:8 InterpolationEnd """#,
            r#"1:8 TextEnd """#,
            r#"1:8 InterpolationEnd """#,
            r#"1:8 TextEnd """#,
            r#"1:8 Newline "\n""#,
            r#"2:1 Plus "+""#,
            r#"2:2 Eof """#,
        ],
        &[UnterminatedInterpolation(5..7)],
    );
    assert_lexed(
        "\"{f(\"ab\n+",
        &[
            r#"1:1 TextStart "\"""#,
            r#"1:2 InterpolationStart "{""#,
            r#"1:3 Identifier "f""#,
            r#"1:4 OpenParen "(""#,
            r#"1:5 TextStart "\"""#,
            r#"1:6 TextPiece("ab") "ab""#,
            r#"1:8 TextEnd """#,
            r#"1:8 InterpolationEnd """#,
            r#"1:8 TextEnd """#,
            r#"1:8 Newline "\n""#,
            r#"2:1 Plus "+""#,
            r#"2:2 Eof """#,
        ],
        &[UnterminatedText(4..7)],
    );
    // A raw text or a character literal reports it too.
    assert_lexed(
        "\"{r\"a\n+",
        &[
            r#"1:1 TextStart "\"""#,
            r#"1:2 InterpolationStart "{""#,
            r#"1:3 RawText("a") "r\"a""#,
            r#"1:6 InterpolationEnd """#,
            r#"1:6 TextEnd """#,
            r#"1:6 Newline "\n""#,
            r#"2:1 Plus "+""#,
            r#"2:2 Eof """#,
        ],
        &[UnterminatedRawText(2..5)],
    );
    assert_lexed(
        "\"{'a\n+",
        &[
            r#"1:1 TextStart "\"""#,
            r#"1:2 InterpolationStart "{""#,
            r#"1:3 Char('�') "'a""#,
            r#"1:5 InterpolationEnd """#,
            r#"1:5 TextEnd """#,
            r#"1:5 Newline "\n""#,
            r#"2:1 Plus "+""#,
            r#"2:2 Eof """#,
        ],
        &[UnterminatedCharacterLiteral(2..4)],
    );
    // A text closed before the line end is not cut: the interpolation reports it.
    assert_lexed(
        "\"{\"a\"\n+",
        &[
            r#"1:1 TextStart "\"""#,
            r#"1:2 InterpolationStart "{""#,
            r#"1:3 TextStart "\"""#,
            r#"1:4 TextPiece("a") "a""#,
            r#"1:5 TextEnd "\"""#,
            r#"1:6 InterpolationEnd """#,
            r#"1:6 TextEnd """#,
            r#"1:6 Newline "\n""#,
            r#"2:1 Plus "+""#,
            r#"2:2 Eof """#,
        ],
        &[UnterminatedInterpolation(1..5)],
    );
    // Each line end counts alone: the cut of the first line does not silence the second.
    assert_lexed(
        "\"a\n\"{b",
        &[
            r#"1:1 TextStart "\"""#,
            r#"1:2 TextPiece("a") "a""#,
            r#"1:3 TextEnd """#,
            r#"1:3 Newline "\n""#,
            r#"2:1 TextStart "\"""#,
            r#"2:2 InterpolationStart "{""#,
            r#"2:3 Identifier "b""#,
            r#"2:4 InterpolationEnd """#,
            r#"2:4 TextEnd """#,
            r#"2:4 Eof """#,
        ],
        &[UnterminatedText(0..2), UnterminatedInterpolation(4..6)],
    );
}

#[test]
fn closing_delimiter_does_not_cross_an_interpolation() {
    // The `}` closes the interpolation and the `(` inside it, not the outer block.
    assert_lexed(
        r#"{ "{ ( } x" }"#,
        &[
            r#"1:1 OpenBrace "{""#,
            r#"1:3 TextStart "\"""#,
            r#"1:4 InterpolationStart "{""#,
            r#"1:6 OpenParen "(""#,
            r#"1:8 InterpolationEnd "}""#,
            r#"1:9 TextPiece(" x") " x""#,
            r#"1:11 TextEnd "\"""#,
            r#"1:13 CloseBrace "}""#,
            r#"1:14 Eof """#,
        ],
        &[],
    );
    // The `)` cannot close the outer `(`: the first line ending gives no token. The second
    // `)` closes it: the next line ending gives one.
    assert_lexed(
        "( \"{ ) }\"\n)\n+",
        &[
            r#"1:1 OpenParen "(""#,
            r#"1:3 TextStart "\"""#,
            r#"1:4 InterpolationStart "{""#,
            r#"1:6 CloseParen ")""#,
            r#"1:8 InterpolationEnd "}""#,
            r#"1:9 TextEnd "\"""#,
            r#"2:1 CloseParen ")""#,
            r#"2:2 Newline "\n""#,
            r#"3:1 Plus "+""#,
            r#"3:2 Eof """#,
        ],
        &[],
    );
}

#[test]
fn deeply_nested_texts_do_not_overflow_the_stack() {
    // Tests run on threads with a small stack: a recursive lexer would overflow here.
    let depth = 10_000;
    let text = format!("{}x{}", "\"{".repeat(depth), "}\"".repeat(depth));

    let mut source_map = SourceMap::new();
    let file = source_map.add(String::from("test.fm"), text);
    let lexed = tokenize(&source_map, file);

    assert!(
        lexed.diagnostics().is_empty(),
        "diagnostics: {:?}",
        lexed.diagnostics()
    );
    // Per level: `TextStart`, `InterpolationStart`, `InterpolationEnd`, `TextEnd`. Then
    // `x` and `Eof`.
    assert_eq!(lexed.tokens().len(), 4 * depth + 2);
}
