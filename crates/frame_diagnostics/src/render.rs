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
    let line_col = file.line_col(span.start());
    let line_number = line_col.line();
    let line_text = sanitize_source(file.line_text(line_number));
    let margin_width = line_number.to_string().len() + 1;
    let invalid_start_offset = line_col.column() - 1;

    let mut render = vec![
        title_line(diagnostic),
        location_line(&format!("{}", file.location(span.start())), margin_width),
        separator(margin_width),
        source_line(&line_text, line_number, margin_width),
        source_sub_line(span, margin_width, invalid_start_offset, message),
        separator(margin_width),
    ];

    if let Some(reason) = reason_line(diagnostic.reason(), 1) {
        render.push(reason);
    }

    if let Some(help) = help_line(diagnostic.help(), 1) {
        render.push(help);
    }

    text_from_lines(&render.iter().map(String::deref).collect::<Vec<&str>>())
}

fn title_line(diagnostic: &Diagnostic) -> String {
    format!("error[{}]: {}", diagnostic.code(), diagnostic.title())
}

fn location_line(location: &str, margin_width: usize) -> String {
    format!("{}-> {location}", " ".repeat(margin_width))
}

fn source_line(line_text: &str, line_number: u32, margin_width: usize) -> String {
    format!("{line_number:<margin_width$}| {line_text}")
}

fn source_sub_line(
    span: &Span,
    margin_width: usize,
    start_offset: u32,
    message: Option<&str>,
) -> String {
    let mut source_line = format!(
        "{}| {}{}",
        " ".repeat(margin_width),
        " ".repeat(start_offset as usize),
        "^".repeat(span.len() as usize)
    );

    if let Some(message) = message {
        let _ = write!(source_line, " {message}");
    }

    source_line
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
mod tests {}
