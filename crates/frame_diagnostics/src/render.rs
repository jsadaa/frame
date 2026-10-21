//! Rendering a diagnostic as text, in the layout of DIA-6.

use std::fmt::Write;
use std::ops::Deref;

use super::{Diagnostic, Primary};
use frame_source::{SourceFile, SourceMap, Span};

/// Renders `diagnostic` as text, in the layout of DIA-6.
///
/// The text has the line `error[CODE]: title`, the location, the line of source with the
/// primary span underlined by `^` and followed by its message, then the reason and the help.
/// Each line, the last one included, ends with a newline. In the line of source, a tab is
/// written as four spaces, and a character that LEX-2 refuses as `�`, so that the source
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
        Primary::File { file } => {
            let file = source_map.file(*file);
            render_file_level_diagnostic(file, diagnostic)
        }
        Primary::Span { span, message } => {
            let file = source_map.file(span.file());
            render_span_level_diagnostic(file, diagnostic, span, message.as_deref())
        }
    }
}

/// Renders a diagnostic about a whole file: without a line of source, and with the margin of
/// a one-digit line number.
fn render_file_level_diagnostic(file: &SourceFile, diagnostic: &Diagnostic) -> String {
    let mut lines = vec![
        title_line(diagnostic),
        location_line(file.name(), 1),
        separator(1),
    ];

    if let Some(reason) = reason_line(diagnostic.reason(), 1) {
        lines.push(reason);
    }

    if let Some(help) = help_line(diagnostic.help(), 1) {
        lines.push(help);
    }

    text_from_lines(&lines.iter().map(String::deref).collect::<Vec<&str>>())
}

/// Renders a diagnostic about a span: the line where the span starts, underlined from the
/// start of the span to its end, or to the end of the line when the span goes on.
fn render_span_level_diagnostic(
    file: &SourceFile,
    diagnostic: &Diagnostic,
    span: &Span,
    message: Option<&str>,
) -> String {
    let start_line_col = file.line_col(span.start());
    let end_line_col = file.line_col(span.end());

    let line_number = start_line_col.line();
    let line_text = file.line_text(line_number);

    // The line number, then a space: the `|` comes after the margin.
    let margin_width = line_number.to_string().len() + 1;
    let underline_start = start_line_col.column() as usize - 1;
    let underline_char_count = if end_line_col.line() > start_line_col.line() {
        line_text.chars().skip(underline_start).count()
    } else {
        (end_line_col.column() as usize - 1) - underline_start
    };

    let reason = reason_line(diagnostic.reason(), margin_width);
    let help = help_line(diagnostic.help(), margin_width);

    let mut lines = vec![
        title_line(diagnostic),
        location_line(&file.location(span.start()).to_string(), margin_width),
        separator(margin_width),
        source_line(&displayed_line(line_text), line_number, margin_width),
        underline_line(
            &underline(line_text, underline_start, underline_char_count),
            message,
            margin_width,
        ),
    ];

    if reason.is_some() || help.is_some() {
        lines.push(separator(margin_width));
    }

    if let Some(reason) = reason {
        lines.push(reason);
    }

    if let Some(help) = help {
        lines.push(help);
    }

    text_from_lines(&lines.iter().map(String::deref).collect::<Vec<&str>>())
}

fn title_line(diagnostic: &Diagnostic) -> String {
    format!("error[{}]: {}", diagnostic.code(), diagnostic.title())
}

fn location_line(location: &str, margin_width: usize) -> String {
    format!("{}--> {location}", " ".repeat(margin_width - 1))
}

fn source_line(line_text: &str, line_number: u32, margin_width: usize) -> String {
    let mut line = format!("{line_number:<margin_width$}|");
    // An empty line ends at the bar: the rendering adds no trailing space.
    if !line_text.is_empty() {
        let _ = write!(line, " {line_text}");
    }

    line
}

fn underline_line(underline: &str, message: Option<&str>, margin_width: usize) -> String {
    let mut line = format!("{}|{}", " ".repeat(margin_width), underline);

    if let Some(message) = message {
        let _ = write!(line, " {message}");
    }

    line
}

/// Returns the spaces, then the `^`, that underline `char_count` characters of `line_text`,
/// after its first `start` characters: a tab is four columns wide, and an empty span gets
/// one `^`.
fn underline(line_text: &str, start: usize, char_count: usize) -> String {
    let space_count = line_text
        .chars()
        .take(start)
        .collect::<String>()
        .chars()
        .count();
    let caret_count = line_text
        .chars()
        .skip(start)
        .take(char_count)
        .collect::<String>()
        .replace('\t', &" ".repeat(4))
        .chars()
        .count()
        .max(1);

    format!(" {}{}", " ".repeat(space_count), "^".repeat(caret_count))
}

fn separator(margin_width: usize) -> String {
    format!("{}|", " ".repeat(margin_width))
}

fn reason_line(reason: Option<&str>, margin_width: usize) -> Option<String> {
    reason.map(|reason| format!("{}= reason: {reason}", " ".repeat(margin_width)))
}

fn help_line(help: Option<&str>, margin_width: usize) -> Option<String> {
    help.map(|help| format!("{}= help: {help}", " ".repeat(margin_width)))
}

fn text_from_lines(lines: &[&str]) -> String {
    let mut text = String::new();
    for line in lines {
        text.push_str(line);
        text.push('\n');
    }
    text
}

/// Returns `line` as it is displayed: each tab as four spaces, so that the `^` can be aligned
/// under it, and each character that LEX-2 refuses as `�`, so that the source cannot control
/// the terminal, as a `\r` or an escape sequence would.
fn displayed_line(line: &str) -> String {
    line.replace('\t', &" ".repeat(4))
        .replace(
            |c: char| c.is_control(),
            &char::REPLACEMENT_CHARACTER.to_string(),
        )
        // The bidirectional marks, and U+FEFF: the loader removed the one at the start of the file.
        .replace(
            |c: char| {
                let code_point = c as u32;
                code_point == 0x061C
                    || code_point == 0x200E
                    || code_point == 0x200F
                    || (0x202A..=0x202E).contains(&code_point)
                    || (0x2066..=0x2069).contains(&code_point)
                    || code_point == 0xFEFF
            },
            &char::REPLACEMENT_CHARACTER.to_string(),
        )
}

#[cfg(test)]
mod tests {
    use crate::{Diagnostic, Prefix, Primary, RuleCode, render};
    use frame_source::{FileId, SourceMap, Span};

    fn prepend_lines(n: usize, text: &str) -> String {
        let lines = "\n".repeat(n);
        format!("{lines}{}\n", text)
    }

    fn find(target: &str, source: &str, file: &FileId) -> Span {
        let matches: Vec<_> = source.match_indices(target).collect();
        let start = matches[0].0 as u32;
        Span::new(*file, start, start + target.len() as u32)
    }

    #[test]
    fn example_1_from_notions() {
        let expected = "error[LEX-23]: `&&` is not an operator
 --> demo.fm:3:6
  |
3 | if a && b {
  |      ^^ use `and`
  |
  = help: write `and` instead of `&&`
";

        let mut source_map = SourceMap::new();
        let source = prepend_lines(2, "if a && b {");
        let file = source_map.add(String::from("demo.fm"), source.clone());

        let diagnostic = Diagnostic::new(
            RuleCode::new(Prefix::Lex, 23),
            "`&&` is not an operator",
            Primary::span_with_message(find("&&", &source, &file), String::from("use `and`")),
        )
        .with_help("write `and` instead of `&&`");

        assert_eq!(render(&diagnostic, &source_map), expected);
    }

    #[test]
    fn example_2_from_notions() {
        let expected = "error[NAM-9]: unknown name `totl`
  --> src/main.fm:12:11
   |
12 |     print(totl)
   |           ^^^^ not found in this scope
";

        let mut source_map = SourceMap::new();
        let source = prepend_lines(11, "    print(totl)");
        let file = source_map.add(String::from("src/main.fm"), source.clone());

        let diagnostic = Diagnostic::new(
            RuleCode::new(Prefix::Nam, 9),
            "unknown name `totl`",
            Primary::span_with_message(
                find("totl", &source, &file),
                String::from("not found in this scope"),
            ),
        );

        assert_eq!(render(&diagnostic, &source_map), expected);
    }

    #[test]
    fn text_with_1000_lines() {
        let expected = "error[NAM-9]: unknown name `totl`
    --> src/main.fm:1000:11
     |
1000 |     print(totl)
     |           ^^^^ not found in this scope
";

        let mut source_map = SourceMap::new();
        let source = prepend_lines(999, "    print(totl)");
        let file = source_map.add(String::from("src/main.fm"), source.clone());

        let diagnostic = Diagnostic::new(
            RuleCode::new(Prefix::Nam, 9),
            "unknown name `totl`",
            Primary::span_with_message(
                find("totl", &source, &file),
                String::from("not found in this scope"),
            ),
        );

        assert_eq!(render(&diagnostic, &source_map), expected);
    }

    #[test]
    fn primary_without_message() {
        let expected = "error[NAM-9]: unknown name `totl`
  --> src/main.fm:12:11
   |
12 |     print(totl)
   |           ^^^^
";

        let mut source_map = SourceMap::new();
        let source = prepend_lines(11, "    print(totl)");
        let file = source_map.add(String::from("src/main.fm"), source.clone());

        let diagnostic = Diagnostic::new(
            RuleCode::new(Prefix::Nam, 9),
            "unknown name `totl`",
            Primary::span(find("totl", &source, &file)),
        );

        assert_eq!(render(&diagnostic, &source_map), expected);
    }

    #[test]
    fn reason_and_help() {
        let expected = "error[NAM-9]: unknown name `totl`
  --> src/main.fm:12:11
   |
12 |     print(totl)
   |           ^^^^
   |
   = reason: the variable `totl` is not declared
   = help: initialize the variable
";

        let mut source_map = SourceMap::new();
        let source = prepend_lines(11, "    print(totl)");
        let file = source_map.add(String::from("src/main.fm"), source.clone());
        let diagnostic = Diagnostic::new(
            RuleCode::new(Prefix::Nam, 9),
            "unknown name `totl`",
            Primary::span(find("totl", &source, &file)),
        )
        .with_reason("the variable `totl` is not declared")
        .with_help("initialize the variable");

        assert_eq!(render(&diagnostic, &source_map), expected);
    }

    #[test]
    fn reason() {
        let expected = "error[NAM-9]: unknown name `totl`
  --> src/main.fm:12:11
   |
12 |     print(totl)
   |           ^^^^
   |
   = reason: the variable `totl` is not declared
";

        let mut source_map = SourceMap::new();
        let source = prepend_lines(11, "    print(totl)");
        let file = source_map.add(String::from("src/main.fm"), source.clone());

        let diagnostic = Diagnostic::new(
            RuleCode::new(Prefix::Nam, 9),
            "unknown name `totl`",
            Primary::span(find("totl", &source, &file)),
        )
        .with_reason("the variable `totl` is not declared");

        assert_eq!(render(&diagnostic, &source_map), expected);
    }

    #[test]
    fn help() {
        let expected = "error[NAM-9]: unknown name `totl`
  --> src/main.fm:12:11
   |
12 |     print(totl)
   |           ^^^^
   |
   = help: initialize the variable
";

        let mut source_map = SourceMap::new();
        let source = prepend_lines(11, "    print(totl)");
        let file = source_map.add(String::from("src/main.fm"), source.clone());

        let diagnostic = Diagnostic::new(
            RuleCode::new(Prefix::Nam, 9),
            "unknown name `totl`",
            Primary::span(find("totl", &source, &file)),
        )
        .with_help("initialize the variable");

        assert_eq!(render(&diagnostic, &source_map), expected);
    }

    #[test]
    fn without_reason_and_help() {
        let expected = "error[NAM-9]: unknown name `totl`
  --> src/main.fm:12:11
   |
12 |     print(totl)
   |           ^^^^ not found in this scope
";

        let mut source_map = SourceMap::new();
        let source = prepend_lines(11, "    print(totl)");
        let file = source_map.add(String::from("src/main.fm"), source.clone());

        let diagnostic = Diagnostic::new(
            RuleCode::new(Prefix::Nam, 9),
            "unknown name `totl`",
            Primary::span_with_message(find("totl", &source, &file), "not found in this scope"),
        );

        assert_eq!(render(&diagnostic, &source_map), expected);
    }

    #[test]
    fn tabulation_inside_span() {
        let expected = "error[NAM-9]: unknown name `totl`
  --> src/main.fm:12:11
   |
12 |     print(t    tl)
   |           ^^^^^^^
";

        let mut source_map = SourceMap::new();
        let source = prepend_lines(11, "    print(t\ttl)");
        let file = source_map.add(String::from("src/main.fm"), source.clone());

        let diagnostic = Diagnostic::new(
            RuleCode::new(Prefix::Nam, 9),
            "unknown name `totl`",
            Primary::span(Span::new(file, 21, 25)),
        );

        assert_eq!(render(&diagnostic, &source_map), expected);
    }

    #[test]
    fn multibyte_char_inside_span() {
        let expected = "error[NAM-9]: unknown name `x`
 --> src/main.fm:1:9
  |
1 | let é = x
  |         ^
";

        let mut source_map = SourceMap::new();
        let source = prepend_lines(0, "let é = x");
        let file = source_map.add(String::from("src/main.fm"), source.clone());

        let diagnostic = Diagnostic::new(
            RuleCode::new(Prefix::Nam, 9),
            "unknown name `x`",
            Primary::span(find("x", &source, &file)),
        );

        assert_eq!(render(&diagnostic, &source_map), expected);
    }

    #[test]
    fn empty_span() {
        let expected = "error[NAM-9]: unknown name `t`
 --> src/main.fm:1:1
  |
1 | t
  | ^
";

        let mut source_map = SourceMap::new();
        let source = prepend_lines(0, "t");
        let file = source_map.add(String::from("src/main.fm"), source.clone());

        let diagnostic = Diagnostic::new(
            RuleCode::new(Prefix::Nam, 9),
            "unknown name `t`",
            Primary::span(Span::new(file, 0, 0)),
        );

        assert_eq!(render(&diagnostic, &source_map), expected);
    }

    #[test]
    fn end_of_file_span() {
        let expected = "error[NAM-9]: unknown name `t`
 --> src/main.fm:2:1
  |
2 |
  | ^
";

        let mut source_map = SourceMap::new();
        let source = prepend_lines(0, "t");
        let file = source_map.add(String::from("src/main.fm"), source.clone());

        let diagnostic = Diagnostic::new(
            RuleCode::new(Prefix::Nam, 9),
            "unknown name `t`",
            Primary::span(Span::new(file, 2, 2)),
        );

        assert_eq!(render(&diagnostic, &source_map), expected);
    }

    #[test]
    fn multi_line_span() {
        let expected = "error[NAM-9]: unknown name `totl`
 --> src/main.fm:1:9
  |
1 | let s = \"a long string on a line\"
  |         ^^^^^^^^^^^^^^^^^^^^^^^^^
";

        let mut source_map = SourceMap::new();
        let source = prepend_lines(0, "let s = \"a long string on a line\"\n print(\"s\")");
        let file = source_map.add(String::from("src/main.fm"), source.clone());

        let diagnostic = Diagnostic::new(
            RuleCode::new(Prefix::Nam, 9),
            "unknown name `totl`",
            Primary::span(find(
                "\"a long string on a line\"\n print(\"s\")",
                "let s = \"a long string on a line\"\n print(\"s\")",
                &file,
            )),
        );

        assert_eq!(render(&diagnostic, &source_map), expected);
    }
}
