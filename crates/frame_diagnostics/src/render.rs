//! The diagnostic render module

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
    text_from_lines(
        &[
            Some(title_line(diagnostic)).as_deref(),
            Some(location_line(file.name(), 1)).as_deref(),
            Some(separator(1)).as_deref(),
            reason_line(diagnostic.reason(), 1).as_deref(),
            help_line(diagnostic.help(), 1).as_deref(),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<&str>>(),
    )
}

fn render_span_level_diagnostic(
    file: &SourceFile,
    diagnostic: &Diagnostic,
    span: &Span,
    message: Option<&str>,
) -> String {
    let line_col = file.line_col(span.start());
    let line_text = sanitize_source(file.line_text(line_col.line()));
    let margin_width = line_col.line().to_string().len() + 1;
    let underline_start = line_col.column() - 1;

    text_from_lines(
        &[
            Some(title_line(diagnostic)).as_deref(),
            Some(location_line(
                &format!("{}", file.location(span.start())),
                margin_width,
            ))
            .as_deref(),
            Some(separator(margin_width)).as_deref(),
            Some(source_line(
                &format!(" {}", line_text),
                &format!("{}", line_col.line()),
                margin_width,
            ))
            .as_deref(),
            Some(target_line(
                &line_text,
                span,
                margin_width,
                underline_start,
                message,
            ))
            .as_deref(),
            Some(separator(margin_width)).as_deref(),
            reason_line(diagnostic.reason(), margin_width).as_deref(),
            help_line(diagnostic.help(), margin_width).as_deref(),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<&str>>(),
    )
}

fn title_line(diagnostic: &Diagnostic) -> String {
    format!("error[{}]: {}", diagnostic.code(), diagnostic.title())
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
