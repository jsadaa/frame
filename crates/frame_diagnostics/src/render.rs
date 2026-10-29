//! Rendering a diagnostic as text, in the layout of DIA-6.

use super::{Diagnostic, Primary};
use frame_source::{Location, SourceFile, SourceMap, Span};

const FILE_LEVEL_MARGIN_WIDTH: usize = 2;
const TAB_WIDTH: usize = 4;

/// Renders `diagnostic` as text, in the layout of DIA-6.
///
/// The text has the line `error[CODE]: title`, the location, the line of source with the
/// primary span underlined by `^` and followed by its message, then the reason and the help.
/// For a whole file, the location is the name of the file alone, and no line of source is
/// shown. Each line, the last one included, ends with a newline. In the line of source, a tab
/// is written as four spaces, and a character that LEX-2 refuses as `�`, so that the source
/// cannot control the terminal.
///
/// Secondary labels and the fix are not rendered yet.
///
/// # Examples
///
/// ```
/// use frame_diagnostics::{Diagnostic, Prefix, Primary, RuleCode, render};
/// use frame_source::{SourceMap, Span};
///
/// let mut source_map = SourceMap::new();
/// let file = source_map.add(String::from("demo.fm"), String::from("if a && b {\n}\n"));
/// let diagnostic = Diagnostic::new(
///     RuleCode::new(Prefix::Lex, 23),
///     "`&&` is not an operator",
///     Primary::span_with_message(Span::new(file, 5, 7), "use `and`"),
/// )
/// .with_help("write `and` instead of `&&`");
///
/// assert_eq!(
///     render(&diagnostic, &source_map),
///     concat!(
///         "error[LEX-23]: `&&` is not an operator\n",
///         " --> demo.fm:1:6\n",
///         "  |\n",
///         "1 | if a && b {\n",
///         "  |      ^^ use `and`\n",
///         "  |\n",
///         "  = help: write `and` instead of `&&`\n",
///     )
/// );
/// ```
///
/// # Panics
///
/// Panics if the file of the primary location is unknown to `source_map`, or if the span of
/// the primary location ends past the end of its text or splits a character.
pub fn render(diagnostic: &Diagnostic, source_map: &SourceMap) -> String {
    match diagnostic.primary() {
        Primary::File { file } => render_file_diagnostic(source_map.file(*file), diagnostic),
        Primary::Span { span, message } => render_span_diagnostic(
            source_map.file(span.file()),
            diagnostic,
            *span,
            message.as_deref(),
        ),
    }
}

/// Renders a diagnostic about a whole file: without a line of source, and with the margin of
/// a one-digit line number.
fn render_file_diagnostic(file: &SourceFile, diagnostic: &Diagnostic) -> String {
    let mut lines = vec![
        title_line(diagnostic),
        location_line(file.name(), FILE_LEVEL_MARGIN_WIDTH),
    ];
    lines.append(&mut end_lines(diagnostic, FILE_LEVEL_MARGIN_WIDTH));

    text_from_lines(&lines)
}

/// Renders a diagnostic about a span: the line where the span starts, underlined from the
/// start of the span to its end, or to the end of the line when the span goes on.
fn render_span_diagnostic(
    file: &SourceFile,
    diagnostic: &Diagnostic,
    span: Span,
    message: Option<&str>,
) -> String {
    let start_line_col = file.line_col(span.start());
    let end_line_col = file.line_col(span.end());

    let line_number = start_line_col.line();
    let line_number_digit_count = line_number.to_string().len();
    let line_text = file.line_text(line_number);

    // The line number, then a space: the `|` comes after the margin.
    let margin_width = line_number_digit_count + 1;

    let underline_start = start_line_col.column() as usize - 1;
    let underline_char_count = if end_line_col.line() > start_line_col.line() {
        line_text.chars().count()
    } else {
        (end_line_col.column() as usize - 1) - underline_start
    };

    let mut lines = vec![
        title_line(diagnostic),
        location_line(
            &Location::new(file.name(), start_line_col).to_string(),
            margin_width,
        ),
        separator(margin_width),
        source_line(line_text, line_number, line_number_digit_count),
        underline_line(
            &underline(line_text, underline_start, underline_char_count),
            message,
            margin_width,
        ),
    ];
    lines.extend(end_lines(diagnostic, margin_width));

    text_from_lines(&lines)
}

fn title_line(diagnostic: &Diagnostic) -> String {
    format!(
        "error[{}]: {}",
        diagnostic.code(),
        displayed_text(diagnostic.title())
    )
}

fn location_line(location: &str, margin_width: usize) -> String {
    format!("{}--> {location}", " ".repeat(margin_width - 1))
}

fn source_line(line_text: &str, line_number: u32, line_number_digit_count: usize) -> String {
    let mut line = format!("{line_number:>line_number_digit_count$} |");
    // An empty line ends at the bar: the rendering adds no trailing space.
    if !line_text.is_empty() {
        line.push(' ');
        line.push_str(&displayed_text(line_text));
    }

    line
}

fn underline_line(underline: &str, message: Option<&str>, margin_width: usize) -> String {
    let mut line = format!("{}| {}", " ".repeat(margin_width), underline);

    if let Some(message) = message
        && !message.is_empty()
    {
        line.push(' ');
        line.push_str(&displayed_text(message));
    }

    line
}

/// Returns what follows the `|` under the line of source: a space, then the spaces and the `^`
/// that underline `char_count` characters of `line_text`, after its first `start` characters.
/// A tab is four columns wide, and an empty span gets one `^`.
fn underline(line_text: &str, start: usize, char_count: usize) -> String {
    let space_count = display_width(line_text, 0, start);
    let caret_count = display_width(line_text, start, char_count).max(1);

    format!("{}{}", " ".repeat(space_count), "^".repeat(caret_count))
}

fn end_lines(diagnostic: &Diagnostic, margin_width: usize) -> Vec<String> {
    let mut lines: Vec<String> = vec![];
    let reason = diagnostic
        .reason()
        .map(|reason| end_line(reason, "reason", margin_width));
    let help = diagnostic
        .help()
        .map(|help| end_line(help, "help", margin_width));

    if reason.is_some() || help.is_some() {
        lines.push(separator(margin_width));
    }

    if let Some(reason) = reason {
        lines.push(reason);
    }

    if let Some(help) = help {
        lines.push(help);
    }

    lines
}

fn separator(margin_width: usize) -> String {
    format!("{}|", " ".repeat(margin_width))
}

fn end_line(text: &str, kind: &str, margin_width: usize) -> String {
    format!(
        "{}= {kind}: {}",
        " ".repeat(margin_width),
        displayed_text(text)
    )
}

fn text_from_lines(lines: &[String]) -> String {
    let mut text = String::new();
    for line in lines {
        text.push_str(line);
        text.push('\n');
    }
    text
}

/// Returns `text` as it is displayed: each tab as four spaces, so that the `^` can be aligned
/// under it, and each character that LEX-2 refuses as `�`, so that the source cannot control
/// the terminal, as a `\r` or an escape sequence would.
fn displayed_text(text: &str) -> String {
    let mut displayed = String::new();
    for char in text.chars() {
        match char {
            '\t' => displayed.push_str(&" ".repeat(TAB_WIDTH)),
            char if is_refused_by_lex_2(char) => {
                displayed.push(char::REPLACEMENT_CHARACTER);
            }
            char => displayed.push(char),
        }
    }

    displayed
}

const fn is_refused_by_lex_2(char: char) -> bool {
    char.is_control()
        || matches!(char, '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}')
}

fn display_width(text: &str, from: usize, to: usize) -> usize {
    text.chars()
        .skip(from)
        .take(to)
        .map(|char| match char {
            '\t' => TAB_WIDTH,
            _ => 1,
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use crate::{Diagnostic, Prefix, Primary, RuleCode, render};
    use frame_source::{FileId, SourceMap, Span};

    const TRANSLATION_LIMITS: RuleCode = RuleCode::new(Prefix::Cnf, 6);
    const FORBIDDEN_CHARACTER: RuleCode = RuleCode::new(Prefix::Lex, 2);
    const REFUSED_TOKEN: RuleCode = RuleCode::new(Prefix::Lex, 23);
    const UNEXPECTED_CHARACTER: RuleCode = RuleCode::new(Prefix::Lex, 24);
    const GRAMMAR: RuleCode = RuleCode::new(Prefix::Syn, 1);
    const NON_ASSOCIATIVE_OPERATOR: RuleCode = RuleCode::new(Prefix::Syn, 18);
    const UNQUALIFIED_NAME: RuleCode = RuleCode::new(Prefix::Nam, 9);
    const ARGUMENTS: RuleCode = RuleCode::new(Prefix::Exp, 14);

    /// Returns a source map that holds one file, and the id of that file.
    fn source_map_with_file(name: &str, text: &str) -> (SourceMap, FileId) {
        let mut source_map = SourceMap::new();
        let file = source_map.add(name.to_string(), text.to_string());
        (source_map, file)
    }

    /// Returns a text whose line `line_number` is `line`, after empty lines; it ends with a
    /// newline.
    fn text_with_line(line_number: usize, line: &str) -> String {
        format!("{}{line}\n", "\n".repeat(line_number - 1))
    }

    /// Returns the span of the first `excerpt` in the text of `file`.
    fn span_of(excerpt: &str, source_map: &SourceMap, file: FileId) -> Span {
        let text = source_map.file(file).text();
        let Some(start) = text.find(excerpt) else {
            panic!("{excerpt:?} is not in {text:?}");
        };
        let to_offset =
            |index: usize| u32::try_from(index).expect("add() checked that offsets fit in u32");

        Span::new(file, to_offset(start), to_offset(start + excerpt.len()))
    }

    /// Returns the empty span right after the first `excerpt` in the text of `file`.
    fn empty_span_after(excerpt: &str, source_map: &SourceMap, file: FileId) -> Span {
        let end = span_of(excerpt, source_map, file).end();
        Span::new(file, end, end)
    }

    /// Asserts that `diagnostic` renders as `expected_lines`, each ended by a newline.
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
    fn characters_refused_by_lex_2_are_written_as_replacement_characters() {
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
    fn every_character_refused_by_lex_2_is_written_as_a_replacement_character() {
        // The list of LEX-2, written from the rule rather than from the code. A CR is refused
        // here: no line feed follows it.
        let refused = ('\u{0}'..='\u{1F}')
            .filter(|&c| c != '\t' && c != '\n')
            .chain('\u{7F}'..='\u{9F}')
            .chain(['\u{061C}', '\u{200E}', '\u{200F}'])
            .chain('\u{202A}'..='\u{202E}')
            .chain('\u{2066}'..='\u{2069}')
            .chain(['\u{FEFF}']);

        for c in refused {
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
    fn neighbors_of_the_characters_refused_by_lex_2_are_written_as_they_are() {
        // The character next to each range of LEX-2, outside it: a range that grows is seen too.
        let allowed = [
            '\u{20}', '\u{7E}', '\u{A0}', '\u{061B}', '\u{061D}', '\u{200D}', '\u{2010}',
            '\u{2029}', '\u{202F}', '\u{2065}', '\u{206A}', '\u{FEFE}', '\u{FF00}',
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
    fn characters_refused_by_lex_2_in_the_texts_are_written_as_replacement_characters() {
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
}
