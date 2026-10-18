//! The diagnostic render module

use super::{Diagnostic, Primary};
use frame_source::{SourceMap, Span};

/// Render the diagnostic
pub fn render(diagnostic: &Diagnostic, source_map: &SourceMap) -> String {
    let file = source_map.file(match diagnostic.primary() {
        Primary::File { file } => *file,
        Primary::Span { span, .. } => span.file(),
    });

    let code_title_line = format!("error[{}]: {}", diagnostic.code(), diagnostic.title());
    let (location_line, source_line, target_line, reason_line, help_line) =
        match diagnostic.primary() {
            Primary::File { .. } => (
                location_line(file.name(), 1),
                source_line("", "", 1),
                None,
                reason_line(diagnostic.reason(), 1),
                help_line(diagnostic.help(), 1),
            ),
            Primary::Span { span, message } => {
                let line_col = file.line_col(span.start());
                let line_text = sanitize_source(file.line_text(line_col.line()));
                let margin_width = line_col.line().to_string().len() + 1;
                let underline_start = line_col.column() - 1;

                (
                    location_line(&format!("{}", file.location(span.start())), margin_width),
                    source_line(
                        &format!(" {}", line_text),
                        &format!("{}", line_col.line()),
                        margin_width,
                    ),
                    Some(target_line(
                        &line_text,
                        span,
                        margin_width,
                        underline_start,
                        message.as_deref(),
                    )),
                    reason_line(diagnostic.reason(), margin_width),
                    help_line(diagnostic.help(), margin_width),
                )
            }
        };

    let mut render = vec![code_title_line, location_line, source_line];
    for line in [target_line, reason_line, help_line].into_iter().flatten() {
        render.push(line);
    }

    text_from_lines(
        &render
            .iter()
            .map(|line| line.as_str())
            .collect::<Vec<&str>>(),
    )
}

fn location_line(location: &str, margin_width: usize) -> String {
    format!("{}-> {location}", " ".repeat(margin_width))
}

fn source_line(line_text: &str, margin_text: &str, margin_width: usize) -> String {
    format!("{}|{line_text}", source_prefix(margin_text, margin_width))
}

fn source_prefix(margin_text: &str, margin_width: usize) -> String {
    format!("{margin_text:<margin_width$}")
}

fn target_line(
    line_text: &str,
    span: &Span,
    margin_width: usize,
    start_offset: u32,
    message: Option<&str>,
) -> String {
    let underline_start = line_text
        .get(..start_offset as usize)
        .expect("at this stage, we know the column is in the line text range")
        .len();

    let mut target_line = format!(
        "{}| {}{}",
        " ".repeat(margin_width),
        " ".repeat(underline_start),
        "^".repeat(span.len() as usize)
    );

    if let Some(message) = message {
        target_line.push_str(&format!(" {}", message));
    }

    target_line
}

fn reason_line(reason: Option<&str>, margin_width: usize) -> Option<String> {
    reason.map(|r| format!("{}= reason: {}", " ".repeat(margin_width), r))
}

fn help_line(help: Option<&str>, margin_width: usize) -> Option<String> {
    help.map(|r| format!("{}= help: {}", " ".repeat(margin_width), r))
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
        .replace("\t", &" ".repeat(4))
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
