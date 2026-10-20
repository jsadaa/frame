//! The diagnostic render module

use std::fmt::Write;
use std::ops::Deref;

use super::{Diagnostic, Primary};
use frame_source::{SourceFile, SourceMap, Span};

/// Render the diagnostic
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

fn render_file_level_diagnostic(file: &SourceFile, diagnostic: &Diagnostic) -> String {
    let mut render = vec![
        title_line(diagnostic),
        location_line(file.name(), 1),
        separator(1),
    ];

    if let Some(reason) = reason_line(diagnostic.reason(), 1) {
        render.push(reason);
    }

    if let Some(help) = help_line(diagnostic.help(), 1) {
        render.push(help);
    }

    text_from_lines(&render.iter().map(String::deref).collect::<Vec<&str>>())
}

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

    let margin_width = line_number.to_string().len() + 1;
    let underline_start = start_line_col.column() as usize - 1;
    let underline_width = if end_line_col.line() > start_line_col.line() {
        line_text.chars().skip(underline_start).count()
    } else {
        (end_line_col.column() as usize - 1) - underline_start
    };

    let reason = reason_line(diagnostic.reason(), margin_width);
    let help = help_line(diagnostic.help(), margin_width);

    let mut render = vec![
        title_line(diagnostic),
        location_line(&file.location(span.start()).to_string(), margin_width),
        separator(margin_width),
        source_line(&sanitize_source(line_text), line_number, margin_width),
        source_sub_line(
            &underline_text(line_text, underline_start, underline_width),
            message,
            margin_width,
        ),
    ];

    if reason.is_some() || help.is_some() {
        render.push(separator(margin_width));
    }

    if let Some(reason) = reason {
        render.push(reason);
    }

    if let Some(help) = help {
        render.push(help);
    }

    text_from_lines(&render.iter().map(String::deref).collect::<Vec<&str>>())
}

fn title_line(diagnostic: &Diagnostic) -> String {
    format!("error[{}]: {}", diagnostic.code(), diagnostic.title())
}

fn location_line(location: &str, margin_width: usize) -> String {
    format!("{}--> {location}", " ".repeat(margin_width - 1))
}

fn source_line(line_text: &str, line_number: u32, margin_width: usize) -> String {
    let mut source_line = format!("{line_number:<margin_width$}|");
    if !line_text.is_empty() {
        let _ = write!(source_line, " {line_text}");
    }

    source_line
}

fn source_sub_line(line_text: &str, message: Option<&str>, margin_width: usize) -> String {
    let mut source_line = format!("{}|{}", " ".repeat(margin_width), line_text);

    if let Some(message) = message {
        let _ = write!(source_line, " {message}");
    }

    source_line
}

fn underline_text(source_line_text: &str, start: usize, width: usize) -> String {
    let before_count = source_line_text
        .chars()
        .take(start)
        .collect::<String>()
        .chars()
        .count();
    let inside_count = source_line_text
        .chars()
        .skip(start)
        .take(width)
        .collect::<String>()
        .replace('\t', &" ".repeat(4))
        .chars()
        .count()
        .max(1);

    format!(" {}{}", " ".repeat(before_count), "^".repeat(inside_count))
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

fn sanitize_source(source: &str) -> String {
    source
        .replace('\t', &" ".repeat(4))
        .replace(
            |c: char| c.is_control(),
            &char::REPLACEMENT_CHARACTER.to_string(),
        )
        .replace(
            |c: char| {
                let cp = c as u32;
                cp == 0x061C
                    || cp == 0x200E
                    || cp == 0x200F
                    || (0x202A..=0x202E).contains(&cp)
                    || (0x2066..=0x2069).contains(&cp)
                    || cp == 0xFEFF
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
