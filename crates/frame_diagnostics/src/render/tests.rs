use crate::{Diagnostic, Fix, Label, Prefix, Primary, RuleCode, render};
use frame_source::{FileId, SourceMap, Span};

const TRANSLATION_LIMITS: RuleCode = RuleCode::new(Prefix::Cnf, 6);
const FORBIDDEN_CHARACTER: RuleCode = RuleCode::new(Prefix::Lex, 2);
const REFUSED_TOKEN: RuleCode = RuleCode::new(Prefix::Lex, 23);
const UNEXPECTED_CHARACTER: RuleCode = RuleCode::new(Prefix::Lex, 24);
const GRAMMAR: RuleCode = RuleCode::new(Prefix::Syn, 1);
const TOP_LEVEL_STATEMENTS: RuleCode = RuleCode::new(Prefix::Syn, 7);
const CLAUSE_ORDER: RuleCode = RuleCode::new(Prefix::Syn, 8);
const NON_ASSOCIATIVE_OPERATOR: RuleCode = RuleCode::new(Prefix::Syn, 18);
const UNIQUENESS: RuleCode = RuleCode::new(Prefix::Nam, 1);
const UNQUALIFIED_NAME: RuleCode = RuleCode::new(Prefix::Nam, 9);
const MUTATION_MARKS: RuleCode = RuleCode::new(Prefix::Mut, 3);
const ARGUMENT_EXCLUSIVITY: RuleCode = RuleCode::new(Prefix::Mut, 4);
const ARGUMENTS: RuleCode = RuleCode::new(Prefix::Exp, 14);
const EXHAUSTIVENESS: RuleCode = RuleCode::new(Prefix::Pat, 15);
const USELESS_TRY: RuleCode = RuleCode::new(Prefix::Err, 14);

fn source_map_with_file(name: &str, text: &str) -> (SourceMap, FileId) {
    let mut source_map = SourceMap::new();
    let file = source_map.add(name.to_string(), text.to_string());
    (source_map, file)
}

/// Returns a text whose line `line_number` is `line`, preceded by empty lines and
/// followed by a newline.
fn text_with_line(line_number: usize, line: &str) -> String {
    format!("{}{line}\n", "\n".repeat(line_number - 1))
}

/// Returns the span of the first `excerpt` in `file`.
fn span_of(excerpt: &str, source_map: &SourceMap, file: FileId) -> Span {
    let text = source_map.file(file).text();
    let Some(start) = text.find(excerpt) else {
        panic!("{excerpt:?} is not in {text:?}");
    };

    span_at(file, start, excerpt)
}

fn last_span_of(excerpt: &str, source_map: &SourceMap, file: FileId) -> Span {
    let text = source_map.file(file).text();
    let Some(start) = text.rfind(excerpt) else {
        panic!("{excerpt:?} is not in {text:?}");
    };

    span_at(file, start, excerpt)
}

/// Returns the span of `excerpt`, found at byte `start` in `file`.
fn span_at(file: FileId, start: usize, excerpt: &str) -> Span {
    let to_offset =
        |index: usize| u32::try_from(index).expect("add() checked that offsets fit in u32");

    Span::new(file, to_offset(start), to_offset(start + excerpt.len()))
}

/// Returns the empty span right after the first `excerpt` in `file`.
fn empty_span_after(excerpt: &str, source_map: &SourceMap, file: FileId) -> Span {
    let end = span_of(excerpt, source_map, file).end();
    Span::new(file, end, end)
}

/// Asserts that `diagnostic` renders as `expected_lines`, each followed by a newline.
fn assert_render(diagnostic: &Diagnostic, source_map: &SourceMap, expected_lines: &[&str]) {
    let mut expected = String::new();
    for line in expected_lines {
        expected.push_str(line);
        expected.push('\n');
    }

    assert_eq!(render(diagnostic, source_map), expected);
}

#[test]
fn span_with_message_and_help_is_rendered_in_full() {
    let (source_map, file) = source_map_with_file("demo.fm", &text_with_line(3, "if a && b {"));
    let diagnostic = Diagnostic::new(
        REFUSED_TOKEN,
        "`&&` is not an operator",
        Primary::span_with_message(span_of("&&", &source_map, file), "use `and`"),
    )
    .with_help("write `and` instead of `&&`");

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[LEX-23]: `&&` is not an operator",
            " --> demo.fm:3:6",
            "  |",
            "3 | if a && b {",
            "  |      ^^ use `and`",
            "  |",
            "  = help: write `and` instead of `&&`",
        ],
    );
}

#[test]
fn two_digit_line_number_widens_the_margin() {
    let (source_map, file) =
        source_map_with_file("src/main.fm", &text_with_line(12, "    print(totl)"));
    let diagnostic = Diagnostic::new(
        UNQUALIFIED_NAME,
        "unknown name `totl`",
        Primary::span_with_message(
            span_of("totl", &source_map, file),
            "not found in this scope",
        ),
    );

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[NAM-9]: unknown name `totl`",
            "  --> src/main.fm:12:11",
            "   |",
            "12 |     print(totl)",
            "   |           ^^^^ not found in this scope",
        ],
    );
}

#[test]
fn four_digit_line_number_widens_the_margin() {
    let (source_map, file) =
        source_map_with_file("src/main.fm", &text_with_line(1000, "    print(totl)"));
    let diagnostic = Diagnostic::new(
        UNQUALIFIED_NAME,
        "unknown name `totl`",
        Primary::span_with_message(
            span_of("totl", &source_map, file),
            "not found in this scope",
        ),
    );

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[NAM-9]: unknown name `totl`",
            "    --> src/main.fm:1000:11",
            "     |",
            "1000 |     print(totl)",
            "     |           ^^^^ not found in this scope",
        ],
    );
}

#[test]
fn span_without_message_ends_at_the_last_caret() {
    let (source_map, file) =
        source_map_with_file("src/main.fm", &text_with_line(12, "    print(totl)"));
    let diagnostic = Diagnostic::new(
        UNQUALIFIED_NAME,
        "unknown name `totl`",
        Primary::span(span_of("totl", &source_map, file)),
    );

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[NAM-9]: unknown name `totl`",
            "  --> src/main.fm:12:11",
            "   |",
            "12 |     print(totl)",
            "   |           ^^^^",
        ],
    );
}

#[test]
fn span_with_an_empty_message_ends_at_the_last_caret() {
    let (source_map, file) =
        source_map_with_file("src/main.fm", &text_with_line(12, "    print(totl)"));
    let diagnostic = Diagnostic::new(
        UNQUALIFIED_NAME,
        "unknown name `totl`",
        Primary::span_with_message(span_of("totl", &source_map, file), ""),
    );

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[NAM-9]: unknown name `totl`",
            "  --> src/main.fm:12:11",
            "   |",
            "12 |     print(totl)",
            "   |           ^^^^",
        ],
    );
}

#[test]
fn reason_then_help_follow_a_separator() {
    let (source_map, file) =
        source_map_with_file("src/main.fm", &text_with_line(12, "    print(totl)"));
    let diagnostic = Diagnostic::new(
        UNQUALIFIED_NAME,
        "unknown name `totl`",
        Primary::span_with_message(
            span_of("totl", &source_map, file),
            "not found in this scope",
        ),
    )
    .with_reason("no binding, declaration or import is named `totl`")
    .with_help("did you mean `total`?");

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[NAM-9]: unknown name `totl`",
            "  --> src/main.fm:12:11",
            "   |",
            "12 |     print(totl)",
            "   |           ^^^^ not found in this scope",
            "   |",
            "   = reason: no binding, declaration or import is named `totl`",
            "   = help: did you mean `total`?",
        ],
    );
}

#[test]
fn reason_alone_follows_a_separator() {
    let (source_map, file) =
        source_map_with_file("src/main.fm", &text_with_line(12, "    print(totl)"));
    let diagnostic = Diagnostic::new(
        UNQUALIFIED_NAME,
        "unknown name `totl`",
        Primary::span_with_message(
            span_of("totl", &source_map, file),
            "not found in this scope",
        ),
    )
    .with_reason("no binding, declaration or import is named `totl`");

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[NAM-9]: unknown name `totl`",
            "  --> src/main.fm:12:11",
            "   |",
            "12 |     print(totl)",
            "   |           ^^^^ not found in this scope",
            "   |",
            "   = reason: no binding, declaration or import is named `totl`",
        ],
    );
}

#[test]
fn help_alone_follows_a_separator() {
    let (source_map, file) =
        source_map_with_file("src/main.fm", &text_with_line(12, "    print(totl)"));
    let diagnostic = Diagnostic::new(
        UNQUALIFIED_NAME,
        "unknown name `totl`",
        Primary::span_with_message(
            span_of("totl", &source_map, file),
            "not found in this scope",
        ),
    )
    .with_help("did you mean `total`?");

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[NAM-9]: unknown name `totl`",
            "  --> src/main.fm:12:11",
            "   |",
            "12 |     print(totl)",
            "   |           ^^^^ not found in this scope",
            "   |",
            "   = help: did you mean `total`?",
        ],
    );
}

#[test]
fn text_ends_at_the_underline_without_reason_or_help() {
    let (source_map, file) = source_map_with_file("demo.fm", &text_with_line(3, "if a && b {"));
    let diagnostic = Diagnostic::new(
        REFUSED_TOKEN,
        "`&&` is not an operator",
        Primary::span_with_message(span_of("&&", &source_map, file), "use `and`"),
    );

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[LEX-23]: `&&` is not an operator",
            " --> demo.fm:3:6",
            "  |",
            "3 | if a && b {",
            "  |      ^^ use `and`",
        ],
    );
}

#[test]
fn tab_before_the_span_shifts_the_carets() {
    let (source_map, file) = source_map_with_file("demo.fm", "\tif a && b {\n");
    let diagnostic = Diagnostic::new(
        REFUSED_TOKEN,
        "`&&` is not an operator",
        Primary::span_with_message(span_of("&&", &source_map, file), "use `and`"),
    );

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[LEX-23]: `&&` is not an operator",
            " --> demo.fm:1:7",
            "  |",
            "1 |     if a && b {",
            "  |          ^^ use `and`",
        ],
    );
}

#[test]
fn tab_inside_the_span_gets_four_carets() {
    let (source_map, file) = source_map_with_file("demo.fm", "if a <\tb < c {\n");
    let diagnostic = Diagnostic::new(
        NON_ASSOCIATIVE_OPERATOR,
        "comparisons cannot be chained",
        Primary::span(span_of("a <\tb < c", &source_map, file)),
    )
    .with_help("write `a < b and b < c`");

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[SYN-18]: comparisons cannot be chained",
            " --> demo.fm:1:4",
            "  |",
            "1 | if a <    b < c {",
            "  |    ^^^^^^^^^^^^",
            "  |",
            "  = help: write `a < b and b < c`",
        ],
    );
}

#[test]
fn two_byte_character_before_the_span_is_one_column() {
    let (source_map, file) = source_map_with_file("demo.fm", "let é = x\n");
    let diagnostic = Diagnostic::new(
        UNQUALIFIED_NAME,
        "unknown name `x`",
        Primary::span(span_of("x", &source_map, file)),
    );

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[NAM-9]: unknown name `x`",
            " --> demo.fm:1:9",
            "  |",
            "1 | let é = x",
            "  |         ^",
        ],
    );
}

#[test]
fn empty_span_gets_one_caret() {
    let (source_map, file) = source_map_with_file("demo.fm", "print(total limit)\n");
    let diagnostic = Diagnostic::new(
        GRAMMAR,
        "expected `,` or `)`",
        Primary::span(empty_span_after("total", &source_map, file)),
    );

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[SYN-1]: expected `,` or `)`",
            " --> demo.fm:1:12",
            "  |",
            "1 | print(total limit)",
            "  |            ^",
        ],
    );
}

#[test]
fn span_after_the_last_newline_is_on_an_empty_line() {
    let (source_map, file) = source_map_with_file("demo.fm", "fn main() {\n    print(total)\n");
    let diagnostic = Diagnostic::new(
        GRAMMAR,
        "expected `}`",
        Primary::span(empty_span_after("print(total)\n", &source_map, file)),
    );

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[SYN-1]: expected `}`",
            " --> demo.fm:3:1",
            "  |",
            "3 |",
            "  | ^",
        ],
    );
}

#[test]
fn span_over_two_lines_is_underlined_to_the_end_of_the_first() {
    let (source_map, file) = source_map_with_file(
        "demo.fm",
        "let area = rect_area(width,\n    height, depth)\n",
    );
    let diagnostic = Diagnostic::new(
        ARGUMENTS,
        "`rect_area` takes 2 arguments, but 3 were given",
        Primary::span(span_of(
            "rect_area(width,\n    height, depth)",
            &source_map,
            file,
        )),
    );

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[EXP-14]: `rect_area` takes 2 arguments, but 3 were given",
            " --> demo.fm:1:12",
            "  |",
            "1 | let area = rect_area(width,",
            "  |            ^^^^^^^^^^^^^^^^",
        ],
    );
}

#[test]
fn span_over_two_lines_from_column_1_is_underlined_to_the_end_of_the_first() {
    let (source_map, file) =
        source_map_with_file("demo.fm", "rect_area(width,\n    height, depth)\n");
    let diagnostic = Diagnostic::new(
        ARGUMENTS,
        "`rect_area` takes 2 arguments, but 3 were given",
        Primary::span(span_of(
            "rect_area(width,\n    height, depth)",
            &source_map,
            file,
        )),
    );

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[EXP-14]: `rect_area` takes 2 arguments, but 3 were given",
            " --> demo.fm:1:1",
            "  |",
            "1 | rect_area(width,",
            "  | ^^^^^^^^^^^^^^^^",
        ],
    );
}

#[test]
fn span_over_two_lines_after_line_1_is_underlined_to_the_end_of_the_first() {
    let (source_map, file) = source_map_with_file(
        "demo.fm",
        "// déjà vu\nlet area = rect_area(width,\n    height, depth)\n",
    );
    let diagnostic = Diagnostic::new(
        ARGUMENTS,
        "`rect_area` takes 2 arguments, but 3 were given",
        Primary::span(span_of(
            "rect_area(width,\n    height, depth)",
            &source_map,
            file,
        )),
    );

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[EXP-14]: `rect_area` takes 2 arguments, but 3 were given",
            " --> demo.fm:2:12",
            "  |",
            "2 | let area = rect_area(width,",
            "  |            ^^^^^^^^^^^^^^^^",
        ],
    );
}

#[test]
fn crlf_is_neither_shown_nor_underlined() {
    let (source_map, file) = source_map_with_file(
        "demo.fm",
        "let area = rect_area(width,\r\n    height, depth)\r\n",
    );
    let diagnostic = Diagnostic::new(
        ARGUMENTS,
        "`rect_area` takes 2 arguments, but 3 were given",
        Primary::span(span_of(
            "rect_area(width,\r\n    height, depth)",
            &source_map,
            file,
        )),
    );

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[EXP-14]: `rect_area` takes 2 arguments, but 3 were given",
            " --> demo.fm:1:12",
            "  |",
            "1 | let area = rect_area(width,",
            "  |            ^^^^^^^^^^^^^^^^",
        ],
    );
}

#[test]
fn characters_forbidden_by_lex_2_are_written_as_replacement_characters() {
    let (source_map, file) =
        source_map_with_file("demo.fm", "print(\"\r\u{1b}\u{202E}\") # done\n");
    let diagnostic = Diagnostic::new(
        UNEXPECTED_CHARACTER,
        "unexpected character `#`",
        Primary::span(span_of("#", &source_map, file)),
    )
    .with_help("a comment starts with `//`");

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[LEX-24]: unexpected character `#`",
            " --> demo.fm:1:14",
            "  |",
            "1 | print(\"\u{FFFD}\u{FFFD}\u{FFFD}\") # done",
            "  |              ^",
            "  |",
            "  = help: a comment starts with `//`",
        ],
    );
}

#[test]
fn every_character_forbidden_by_lex_2_is_written_as_a_replacement_character() {
    // LEX-2's list, copied from the rule rather than the code, so it can catch a mistake
    // there. The CR counts here because no line feed follows it.
    let forbidden = ('\u{0}'..='\u{1F}')
        .filter(|&c| c != '\t' && c != '\n')
        .chain('\u{7F}'..='\u{9F}')
        .chain(['\u{061C}', '\u{200E}', '\u{200F}'])
        .chain('\u{202A}'..='\u{202E}')
        .chain('\u{2066}'..='\u{2069}')
        .chain(['\u{FEFF}']);

    for c in forbidden {
        let (source_map, file) =
            source_map_with_file("demo.fm", &format!("print(\"{c}\") # done\n"));
        let diagnostic = Diagnostic::new(
            UNEXPECTED_CHARACTER,
            "unexpected character `#`",
            Primary::span(span_of("#", &source_map, file)),
        );

        assert_eq!(
            render(&diagnostic, &source_map),
            concat!(
                "error[LEX-24]: unexpected character `#`\n",
                " --> demo.fm:1:12\n",
                "  |\n",
                "1 | print(\"\u{FFFD}\") # done\n",
                "  |            ^\n",
            ),
            "U+{:04X}",
            u32::from(c)
        );
    }
}

#[test]
fn neighbors_of_the_characters_forbidden_by_lex_2_are_written_as_they_are() {
    // The character just outside each LEX-2 range, so that a range that grows is caught.
    let allowed = [
        '\u{20}', '\u{7E}', '\u{A0}', '\u{061B}', '\u{061D}', '\u{200D}', '\u{2010}', '\u{2029}',
        '\u{202F}', '\u{2065}', '\u{206A}', '\u{FEFE}', '\u{FF00}',
    ];

    for c in allowed {
        let (source_map, file) =
            source_map_with_file("demo.fm", &format!("print(\"{c}\") # done\n"));
        let diagnostic = Diagnostic::new(
            UNEXPECTED_CHARACTER,
            "unexpected character `#`",
            Primary::span(span_of("#", &source_map, file)),
        );

        assert_eq!(
            render(&diagnostic, &source_map),
            format!(
                concat!(
                    "error[LEX-24]: unexpected character `#`\n",
                    " --> demo.fm:1:12\n",
                    "  |\n",
                    "1 | print(\"{}\") # done\n",
                    "  |            ^\n",
                ),
                c
            ),
            "U+{:04X}",
            u32::from(c)
        );
    }
}

#[test]
fn characters_forbidden_by_lex_2_in_the_texts_are_written_as_replacement_characters() {
    let (source_map, file) = source_map_with_file("demo.fm", "print(\"\u{202E}\")\n");
    let diagnostic = Diagnostic::new(
        FORBIDDEN_CHARACTER,
        "forbidden character `\u{202E}`",
        Primary::span_with_message(
            span_of("\u{202E}", &source_map, file),
            "`\u{202E}` is a bidirectional mark",
        ),
    )
    .with_reason("`\u{202E}` can show other code than the compiler reads")
    .with_help("remove `\u{202E}`");

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[LEX-2]: forbidden character `\u{FFFD}`",
            " --> demo.fm:1:8",
            "  |",
            "1 | print(\"\u{FFFD}\")",
            "  |        ^ `\u{FFFD}` is a bidirectional mark",
            "  |",
            "  = reason: `\u{FFFD}` can show other code than the compiler reads",
            "  = help: remove `\u{FFFD}`",
        ],
    );
}

#[test]
fn characters_forbidden_by_lex_2_in_the_file_name_are_written_as_replacement_characters() {
    // `ESC [2J` would clear the screen.
    let (source_map, file) = source_map_with_file("a\u{1b}[2Jb.fm", "if a && b {\n");
    let diagnostic = Diagnostic::new(
        REFUSED_TOKEN,
        "`&&` is not an operator",
        Primary::span_with_message(span_of("&&", &source_map, file), "use `and`"),
    );

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[LEX-23]: `&&` is not an operator",
            " --> a\u{FFFD}[2Jb.fm:1:6",
            "  |",
            "1 | if a && b {",
            "  |      ^^ use `and`",
        ],
    );
}

#[test]
fn whole_file_has_no_source_line() {
    let (source_map, file) = source_map_with_file("big.fm", "");
    let diagnostic = Diagnostic::new(
        TRANSLATION_LIMITS,
        "source file is too large",
        Primary::file(file),
    )
    .with_reason("the limit is 16 MiB (16777216 bytes)");

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[CNF-6]: source file is too large",
            " --> big.fm",
            "  |",
            "  = reason: the limit is 16 MiB (16777216 bytes)",
        ],
    );
}

#[test]
fn whole_file_help_alone_follows_a_separator() {
    let (source_map, file) = source_map_with_file("big.fm", "");
    let diagnostic = Diagnostic::new(
        TRANSLATION_LIMITS,
        "source file is too large",
        Primary::file(file),
    )
    .with_help("split it into smaller files");

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[CNF-6]: source file is too large",
            " --> big.fm",
            "  |",
            "  = help: split it into smaller files",
        ],
    );
}

#[test]
fn whole_file_reason_then_help_follow_a_separator() {
    let (source_map, file) = source_map_with_file("big.fm", "");
    let diagnostic = Diagnostic::new(
        TRANSLATION_LIMITS,
        "source file is too large",
        Primary::file(file),
    )
    .with_reason("the limit is 16 MiB (16777216 bytes)")
    .with_help("split it into smaller files");

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[CNF-6]: source file is too large",
            " --> big.fm",
            "  |",
            "  = reason: the limit is 16 MiB (16777216 bytes)",
            "  = help: split it into smaller files",
        ],
    );
}

#[test]
fn whole_file_without_reason_or_help_ends_at_the_location() {
    let (source_map, file) = source_map_with_file("big.fm", "");
    let diagnostic = Diagnostic::new(
        TRANSLATION_LIMITS,
        "source file is too large",
        Primary::file(file),
    );

    assert_render(
        &diagnostic,
        &source_map,
        &["error[CNF-6]: source file is too large", " --> big.fm"],
    );
}

#[test]
fn mut_4_example_of_dia_6_is_rendered_exactly() {
    let (source_map, file) = source_map_with_file(
        "src/geometry.fm",
        &text_with_line(12, "    swap(mut xs[i], mut xs[j])"),
    );
    let diagnostic = Diagnostic::new(
        ARGUMENT_EXCLUSIVITY,
        "overlapping mutable arguments",
        Primary::span_with_message(span_of("xs[j]", &source_map, file), "modified here"),
    )
    .with_label(Label::new(
        span_of("xs[i]", &source_map, file),
        "also modified here",
    ))
    .with_reason("two indices can designate the same element")
    .with_help("use `xs.swap!(i, j)`");

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[MUT-4]: overlapping mutable arguments",
            "  --> src/geometry.fm:12:25",
            "   |",
            "12 |     swap(mut xs[i], mut xs[j])",
            "   |              ----- also modified here",
            "   |                         ^^^^^ modified here",
            "   |",
            "   = reason: two indices can designate the same element",
            "   = help: use `xs.swap!(i, j)`",
        ],
    );
}

#[test]
fn lex_23_example_with_a_fix_is_rendered_exactly() {
    let (source_map, file) = source_map_with_file("demo.fm", &text_with_line(3, "if a && b {"));
    let diagnostic = Diagnostic::new(
        REFUSED_TOKEN,
        "`&&` is not an operator",
        Primary::span_with_message(span_of("&&", &source_map, file), "use `and`"),
    )
    .with_help("write `and` instead of `&&`")
    .with_fix(Fix::new(span_of("&&", &source_map, file), "and"));

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[LEX-23]: `&&` is not an operator",
            " --> demo.fm:3:6",
            "  |",
            "3 | if a && b {",
            "  |      ^^ use `and`",
            "  |",
            "  = help: write `and` instead of `&&`",
            "  = fix:",
            "3 | if a and b {",
            "  |      ~~~",
        ],
    );
}

#[test]
fn label_on_the_previous_line_is_shown_without_an_ellipsis() {
    let (source_map, file) = source_map_with_file(
        "demo.fm",
        "fn main() {\n    let width = 3\n    let width = 4\n}\n",
    );
    let diagnostic = Diagnostic::new(
        UNIQUENESS,
        "`width` is declared twice",
        Primary::span_with_message(
            last_span_of("width", &source_map, file),
            "declared again here",
        ),
    )
    .with_label(Label::new(
        span_of("width", &source_map, file),
        "first declared here",
    ));

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[NAM-1]: `width` is declared twice",
            " --> demo.fm:3:9",
            "  |",
            "2 |     let width = 3",
            "  |         ----- first declared here",
            "3 |     let width = 4",
            "  |         ^^^^^ declared again here",
        ],
    );
}

#[test]
fn label_five_lines_above_is_separated_by_an_ellipsis() {
    let (source_map, file) = source_map_with_file(
        "demo.fm",
        concat!(
            "fn main() {\n",
            "    let width = 3\n",
            "    print(1)\n",
            "    print(2)\n",
            "    print(3)\n",
            "    print(4)\n",
            "    let width = 4\n",
            "}\n",
        ),
    );
    let diagnostic = Diagnostic::new(
        UNIQUENESS,
        "`width` is declared twice",
        Primary::span_with_message(
            last_span_of("width", &source_map, file),
            "declared again here",
        ),
    )
    .with_label(Label::new(
        span_of("width", &source_map, file),
        "first declared here",
    ));

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[NAM-1]: `width` is declared twice",
            " --> demo.fm:7:9",
            "  |",
            "2 |     let width = 3",
            "  |         ----- first declared here",
            "...",
            "7 |     let width = 4",
            "  |         ^^^^^ declared again here",
        ],
    );
}

#[test]
fn label_two_lines_above_is_separated_by_an_ellipsis() {
    let (source_map, file) = source_map_with_file(
        "demo.fm",
        "fn main() {\n    let width = 3\n    print(width)\n    let width = 4\n}\n",
    );
    let diagnostic = Diagnostic::new(
        UNIQUENESS,
        "`width` is declared twice",
        Primary::span_with_message(
            last_span_of("width", &source_map, file),
            "declared again here",
        ),
    )
    .with_label(Label::new(
        span_of("width", &source_map, file),
        "first declared here",
    ));

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[NAM-1]: `width` is declared twice",
            " --> demo.fm:4:9",
            "  |",
            "2 |     let width = 3",
            "  |         ----- first declared here",
            "...",
            "4 |     let width = 4",
            "  |         ^^^^^ declared again here",
        ],
    );
}

#[test]
fn labels_added_out_of_order_are_shown_in_the_order_of_the_file() {
    let (source_map, file) = source_map_with_file(
        "demo.fm",
        "rotate(\n    mut xs[i],\n    mut xs[j],\n    mut xs[k],\n)\n",
    );
    let diagnostic = Diagnostic::new(
        ARGUMENT_EXCLUSIVITY,
        "overlapping mutable arguments",
        Primary::span_with_message(span_of("xs[j]", &source_map, file), "modified here"),
    )
    .with_label(Label::new(
        span_of("xs[k]", &source_map, file),
        "also modified here",
    ))
    .with_label(Label::new(
        span_of("xs[i]", &source_map, file),
        "also modified here",
    ));

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[MUT-4]: overlapping mutable arguments",
            " --> demo.fm:3:9",
            "  |",
            "2 |     mut xs[i],",
            "  |         ----- also modified here",
            "3 |     mut xs[j],",
            "  |         ^^^^^ modified here",
            "4 |     mut xs[k],",
            "  |         ----- also modified here",
        ],
    );
}

#[test]
fn two_labels_at_the_same_place_keep_the_order_they_were_added() {
    // A sort by message would swap the two labels, since '`' comes before 'a'.
    let (source_map, file) = source_map_with_file("demo.fm", "swap(mut xs[i], mut xs[j])\n");
    let diagnostic = Diagnostic::new(
        ARGUMENT_EXCLUSIVITY,
        "overlapping mutable arguments",
        Primary::span_with_message(span_of("xs[j]", &source_map, file), "modified here"),
    )
    .with_label(Label::new(
        span_of("xs[i]", &source_map, file),
        "also modified here",
    ))
    .with_label(Label::new(
        span_of("xs[i]", &source_map, file),
        "`i` can equal `j`",
    ));

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[MUT-4]: overlapping mutable arguments",
            " --> demo.fm:1:21",
            "  |",
            "1 | swap(mut xs[i], mut xs[j])",
            "  |          ----- also modified here",
            "  |          ----- `i` can equal `j`",
            "  |                     ^^^^^ modified here",
        ],
    );
}

#[test]
fn label_at_the_column_of_the_primary_span_comes_after_the_carets() {
    let (source_map, file) = source_map_with_file("demo.fm", "if a < b < c {\n");
    let diagnostic = Diagnostic::new(
        NON_ASSOCIATIVE_OPERATOR,
        "comparisons cannot be chained",
        Primary::span(span_of("a < b < c", &source_map, file)),
    )
    .with_label(Label::new(
        span_of("a < b", &source_map, file),
        "this comparison is a `bool`",
    ))
    .with_help("write `a < b and b < c`");

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[SYN-18]: comparisons cannot be chained",
            " --> demo.fm:1:4",
            "  |",
            "1 | if a < b < c {",
            "  |    ^^^^^^^^^",
            "  |    ----- this comparison is a `bool`",
            "  |",
            "  = help: write `a < b and b < c`",
        ],
    );
}

#[test]
fn label_over_two_lines_is_underlined_to_the_end_of_the_first() {
    let (source_map, file) = source_map_with_file(
        "demo.fm",
        "let area = rect_area(width,\n    height, depth)\n",
    );
    let diagnostic = Diagnostic::new(
        ARGUMENTS,
        "`rect_area` takes 2 arguments, but 3 were given",
        Primary::span_with_message(span_of("depth", &source_map, file), "unexpected argument"),
    )
    .with_label(Label::new(
        span_of("rect_area(width,\n    height, depth)", &source_map, file),
        "in this call",
    ));

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[EXP-14]: `rect_area` takes 2 arguments, but 3 were given",
            " --> demo.fm:2:13",
            "  |",
            "1 | let area = rect_area(width,",
            "  |            ---------------- in this call",
            "2 |     height, depth)",
            "  |             ^^^^^ unexpected argument",
        ],
    );
}

#[test]
fn characters_forbidden_by_lex_2_in_a_label_are_written_as_replacement_characters() {
    let (source_map, file) = source_map_with_file("demo.fm", "print(\"\u{202E}\")\n");
    let diagnostic = Diagnostic::new(
        FORBIDDEN_CHARACTER,
        "forbidden character `\u{202E}`",
        Primary::span(span_of("\u{202E}", &source_map, file)),
    )
    .with_label(Label::new(
        span_of("\"\u{202E}\"", &source_map, file),
        "`\u{202E}` reverses the text after it",
    ));

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[LEX-2]: forbidden character `\u{FFFD}`",
            " --> demo.fm:1:8",
            "  |",
            "1 | print(\"\u{FFFD}\")",
            "  |       --- `\u{FFFD}` reverses the text after it",
            "  |        ^",
        ],
    );
}

#[test]
fn line_numbers_9_and_10_are_aligned_on_the_right() {
    let (source_map, file) = source_map_with_file(
        "demo.fm",
        &format!("{}let width = 3\nlet width = 4\n", "\n".repeat(8)),
    );
    let diagnostic = Diagnostic::new(
        UNIQUENESS,
        "`width` is declared twice",
        Primary::span_with_message(
            last_span_of("width", &source_map, file),
            "declared again here",
        ),
    )
    .with_label(Label::new(
        span_of("width", &source_map, file),
        "first declared here",
    ));

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[NAM-1]: `width` is declared twice",
            "  --> demo.fm:10:5",
            "   |",
            " 9 | let width = 3",
            "   |     ----- first declared here",
            "10 | let width = 4",
            "   |     ^^^^^ declared again here",
        ],
    );
}

#[test]
fn fix_that_reaches_line_10_widens_the_whole_margin() {
    let (source_map, file) = source_map_with_file(
        "demo.fm",
        &text_with_line(9, "let width = 3; let height = 4"),
    );
    let diagnostic = Diagnostic::new(
        REFUSED_TOKEN,
        "unexpected `;`",
        Primary::span_with_message(
            span_of(";", &source_map, file),
            "write one statement per line",
        ),
    )
    .with_fix(Fix::new(span_of("; ", &source_map, file), "\n"));

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[LEX-23]: unexpected `;`",
            "  --> demo.fm:9:14",
            "   |",
            " 9 | let width = 3; let height = 4",
            "   |              ^ write one statement per line",
            "   |",
            "   = fix:",
            " 9 | let width = 3",
            "10 | let height = 4",
        ],
    );
}

#[test]
fn insertion_is_underlined_by_tildes() {
    let (source_map, file) = source_map_with_file("demo.fm", "apply_discount(order, rate)\n");
    let diagnostic = Diagnostic::new(
        MUTATION_MARKS,
        "argument `order` is modified, but not marked `mut`",
        Primary::span_with_message(
            span_of("order", &source_map, file),
            "`apply_discount` modifies it",
        ),
    )
    .with_fix(Fix::new(
        empty_span_after("apply_discount(", &source_map, file),
        "mut ",
    ));

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[MUT-3]: argument `order` is modified, but not marked `mut`",
            " --> demo.fm:1:16",
            "  |",
            "1 | apply_discount(order, rate)",
            "  |                ^^^^^ `apply_discount` modifies it",
            "  |",
            "  = fix:",
            "1 | apply_discount(mut order, rate)",
            "  |                ~~~~",
        ],
    );
}

#[test]
fn deletion_has_no_tildes() {
    let (source_map, file) = source_map_with_file("demo.fm", "let n = try names.len()\n");
    let diagnostic = Diagnostic::new(
        USELESS_TRY,
        "useless `try`",
        Primary::span_with_message(span_of("try", &source_map, file), "`len` cannot fail"),
    )
    .with_fix(Fix::new(span_of("try ", &source_map, file), ""));

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[ERR-14]: useless `try`",
            " --> demo.fm:1:9",
            "  |",
            "1 | let n = try names.len()",
            "  |         ^^^ `len` cannot fail",
            "  |",
            "  = fix:",
            "1 | let n = names.len()",
        ],
    );
}

#[test]
fn tab_before_the_fix_shifts_the_tildes() {
    let (source_map, file) = source_map_with_file("demo.fm", "\tif a && b {\n");
    let diagnostic = Diagnostic::new(
        REFUSED_TOKEN,
        "`&&` is not an operator",
        Primary::span_with_message(span_of("&&", &source_map, file), "use `and`"),
    )
    .with_fix(Fix::new(span_of("&&", &source_map, file), "and"));

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[LEX-23]: `&&` is not an operator",
            " --> demo.fm:1:7",
            "  |",
            "1 |     if a && b {",
            "  |          ^^ use `and`",
            "  |",
            "  = fix:",
            "1 |     if a and b {",
            "  |          ~~~",
        ],
    );
}

#[test]
fn fix_over_several_lines_shows_them_numbered_without_tildes() {
    let (source_map, file) = source_map_with_file("demo.fm", "let width = 3; let height = 4\n");
    let diagnostic = Diagnostic::new(
        REFUSED_TOKEN,
        "unexpected `;`",
        Primary::span_with_message(
            span_of(";", &source_map, file),
            "write one statement per line",
        ),
    )
    .with_fix(Fix::new(span_of("; ", &source_map, file), "\n"));

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[LEX-23]: unexpected `;`",
            " --> demo.fm:1:14",
            "  |",
            "1 | let width = 3; let height = 4",
            "  |              ^ write one statement per line",
            "  |",
            "  = fix:",
            "1 | let width = 3",
            "2 | let height = 4",
        ],
    );
}

#[test]
fn fix_in_a_crlf_file_shows_neither_cr_nor_replacement_character() {
    let (source_map, file) = source_map_with_file(
        "demo.fm",
        "let width = 3; let height = 4\r\nprint(width)\r\n",
    );
    let diagnostic = Diagnostic::new(
        REFUSED_TOKEN,
        "unexpected `;`",
        Primary::span_with_message(
            span_of(";", &source_map, file),
            "write one statement per line",
        ),
    )
    .with_fix(Fix::new(span_of("; ", &source_map, file), "\r\n"));

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[LEX-23]: unexpected `;`",
            " --> demo.fm:1:14",
            "  |",
            "1 | let width = 3; let height = 4",
            "  |              ^ write one statement per line",
            "  |",
            "  = fix:",
            "1 | let width = 3",
            "2 | let height = 4",
        ],
    );
}

#[test]
fn fix_on_another_line_than_the_primary_span_shows_its_own_line_numbers() {
    let (source_map, file) = source_map_with_file(
        "demo.fm",
        "fn label(op: Op) -> Text = match op {\n    .add => \"+\"\n    .sub => \"-\"\n}\n",
    );
    let diagnostic = Diagnostic::new(
        EXHAUSTIVENESS,
        "`match` is not exhaustive",
        Primary::span_with_message(
            span_of("match op", &source_map, file),
            "`.mul` and `.div` are not matched",
        ),
    )
    .with_fix(Fix::new(
        empty_span_after(".sub => \"-\"\n", &source_map, file),
        "    .mul => \"*\"\n    .div => \"/\"\n",
    ));

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[PAT-15]: `match` is not exhaustive",
            " --> demo.fm:1:28",
            "  |",
            "1 | fn label(op: Op) -> Text = match op {",
            "  |                            ^^^^^^^^ `.mul` and `.div` are not matched",
            "  |",
            "  = fix:",
            "4 |     .mul => \"*\"",
            "5 |     .div => \"/\"",
            "6 | }",
        ],
    );
}

#[test]
fn fix_of_a_span_over_two_lines_on_one_line_has_no_tildes() {
    let (source_map, file) = source_map_with_file(
        "demo.fm",
        "fn read(path: Path) -> Text fails IoError\n    uses fs = fs.read(path)\n",
    );
    let diagnostic = Diagnostic::new(
        CLAUSE_ORDER,
        "clauses out of order",
        Primary::span_with_message(
            span_of("uses fs", &source_map, file),
            "`uses` comes before `fails`",
        ),
    )
    .with_fix(Fix::new(
        span_of("fails IoError\n    uses fs", &source_map, file),
        "uses fs fails IoError",
    ));

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[SYN-8]: clauses out of order",
            " --> demo.fm:2:5",
            "  |",
            "2 |     uses fs = fs.read(path)",
            "  |     ^^^^^^^ `uses` comes before `fails`",
            "  |",
            "  = fix:",
            "1 | fn read(path: Path) -> Text uses fs fails IoError = fs.read(path)",
        ],
    );
}

#[test]
fn tab_in_the_new_text_gets_four_tildes() {
    let (source_map, file) = source_map_with_file(
        "demo.fm",
        "fn read(path: Path) -> Text fails IoError\tuses fs = fs.read(path)\n",
    );
    let diagnostic = Diagnostic::new(
        CLAUSE_ORDER,
        "clauses out of order",
        Primary::span_with_message(
            span_of("uses fs", &source_map, file),
            "`uses` comes before `fails`",
        ),
    )
    .with_fix(Fix::new(
        span_of("fails IoError\tuses fs", &source_map, file),
        "uses fs\tfails IoError",
    ));

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[SYN-8]: clauses out of order",
            " --> demo.fm:1:43",
            "  |",
            "1 | fn read(path: Path) -> Text fails IoError    uses fs = fs.read(path)",
            "  |                                              ^^^^^^^ `uses` comes before `fails`",
            "  |",
            "  = fix:",
            "1 | fn read(path: Path) -> Text uses fs    fails IoError = fs.read(path)",
            "  |                             ~~~~~~~~~~~~~~~~~~~~~~~~",
        ],
    );
}

#[test]
fn fix_without_reason_or_help_follows_a_separator() {
    let (source_map, file) = source_map_with_file("demo.fm", "if a && b {\n");
    let diagnostic = Diagnostic::new(
        REFUSED_TOKEN,
        "`&&` is not an operator",
        Primary::span_with_message(span_of("&&", &source_map, file), "use `and`"),
    )
    .with_fix(Fix::new(span_of("&&", &source_map, file), "and"));

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[LEX-23]: `&&` is not an operator",
            " --> demo.fm:1:6",
            "  |",
            "1 | if a && b {",
            "  |      ^^ use `and`",
            "  |",
            "  = fix:",
            "1 | if a and b {",
            "  |      ~~~",
        ],
    );
}

#[test]
fn reason_then_help_then_fix_follow_a_separator() {
    let (source_map, file) = source_map_with_file("demo.fm", "if a && b {\n");
    let diagnostic = Diagnostic::new(
        REFUSED_TOKEN,
        "`&&` is not an operator",
        Primary::span_with_message(span_of("&&", &source_map, file), "use `and`"),
    )
    .with_reason("logical operators are written as words")
    .with_help("write `and` instead of `&&`")
    .with_fix(Fix::new(span_of("&&", &source_map, file), "and"));

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[LEX-23]: `&&` is not an operator",
            " --> demo.fm:1:6",
            "  |",
            "1 | if a && b {",
            "  |      ^^ use `and`",
            "  |",
            "  = reason: logical operators are written as words",
            "  = help: write `and` instead of `&&`",
            "  = fix:",
            "1 | if a and b {",
            "  |      ~~~",
        ],
    );
}

#[test]
fn whole_file_shows_the_line_of_its_label() {
    let (source_map, file) = source_map_with_file(
        "src/main.fm",
        "fn report() {\n    print(\"report\")\n}\nreport()\n",
    );
    let diagnostic = Diagnostic::new(
        TOP_LEVEL_STATEMENTS,
        "top-level statement in a module file",
        Primary::file(file),
    )
    .with_label(Label::new(
        last_span_of("report()", &source_map, file),
        "runs only in a script",
    ));

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[SYN-7]: top-level statement in a module file",
            " --> src/main.fm",
            "  |",
            "4 | report()",
            "  | -------- runs only in a script",
        ],
    );
}

#[test]
fn whole_file_shows_its_fix() {
    let (source_map, file) = source_map_with_file(
        "src/main.fm",
        "fn report() {\n    print(\"report\")\n}\nreport()\n",
    );
    let diagnostic = Diagnostic::new(
        TOP_LEVEL_STATEMENTS,
        "top-level statement in a module file",
        Primary::file(file),
    )
    .with_fix(Fix::new(
        empty_span_after("}\n", &source_map, file),
        "fn main() = ",
    ));

    assert_render(
        &diagnostic,
        &source_map,
        &[
            "error[SYN-7]: top-level statement in a module file",
            " --> src/main.fm",
            "  |",
            "  = fix:",
            "4 | fn main() = report()",
            "  | ~~~~~~~~~~~~",
        ],
    );
}

#[test]
#[should_panic(
    expected = "a label must be in the file of the primary location, 'src/main.fm', not in 'src/shapes.fm'"
)]
fn rejects_a_label_in_another_file() {
    // The files of a module share one scope (NAM-1).
    let mut source_map = SourceMap::new();
    let main = source_map.add(String::from("src/main.fm"), String::from("let area = 3\n"));
    let shapes = source_map.add(
        String::from("src/shapes.fm"),
        String::from("let area = 4\n"),
    );
    let diagnostic = Diagnostic::new(
        UNIQUENESS,
        "`area` is declared twice",
        Primary::span_with_message(span_of("area", &source_map, main), "declared again here"),
    )
    .with_label(Label::new(
        span_of("area", &source_map, shapes),
        "first declared here",
    ));

    render(&diagnostic, &source_map);
}

#[test]
#[should_panic(
    expected = "a label must be in the file of the primary location, 'src/main.fm', not in 'src/shapes.fm'"
)]
fn rejects_a_label_in_another_file_for_a_whole_file() {
    let mut source_map = SourceMap::new();
    let main = source_map.add(
        String::from("src/main.fm"),
        String::from("fn main() {\n}\n"),
    );
    let shapes = source_map.add(String::from("src/shapes.fm"), String::from("report()\n"));
    let diagnostic = Diagnostic::new(
        TOP_LEVEL_STATEMENTS,
        "top-level statement in a module file",
        Primary::file(main),
    )
    .with_label(Label::new(
        span_of("report()", &source_map, shapes),
        "runs only in a script",
    ));

    render(&diagnostic, &source_map);
}

#[test]
#[should_panic(
    expected = "the fix must be in the file of the primary location, 'src/main.fm', not in 'src/orders.fm'"
)]
fn rejects_a_fix_in_another_file() {
    let mut source_map = SourceMap::new();
    let main = source_map.add(
        String::from("src/main.fm"),
        String::from("apply_discount(order, rate)\n"),
    );
    let orders = source_map.add(
        String::from("src/orders.fm"),
        String::from("apply_discount(order, rate)\n"),
    );
    let diagnostic = Diagnostic::new(
        MUTATION_MARKS,
        "argument `order` is modified, but not marked `mut`",
        Primary::span(span_of("order", &source_map, main)),
    )
    .with_fix(Fix::new(
        empty_span_after("apply_discount(", &source_map, orders),
        "mut ",
    ));

    render(&diagnostic, &source_map);
}

#[test]
#[should_panic(
    expected = "the fix must be in the file of the primary location, 'src/main.fm', not in 'src/orders.fm'"
)]
fn rejects_a_fix_in_another_file_for_a_whole_file() {
    let mut source_map = SourceMap::new();
    let main = source_map.add(String::from("src/main.fm"), String::from("report()\n"));
    let orders = source_map.add(String::from("src/orders.fm"), String::from("report()\n"));
    let diagnostic = Diagnostic::new(
        TOP_LEVEL_STATEMENTS,
        "top-level statement in a module file",
        Primary::file(main),
    )
    .with_fix(Fix::new(
        empty_span_after("", &source_map, orders),
        "fn main() = ",
    ));

    render(&diagnostic, &source_map);
}
