//! Rendering diagnostics as text for the terminal.

use super::{Diagnostic, Fix, Primary};
use frame_source::{FileId, Location, SourceFile, SourceMap, Span};
use std::iter;

/// The margin width when no line is shown, as for a whole file. It matches a one-digit line
/// number.
const FILE_LEVEL_MARGIN_WIDTH: usize = 2;
const TAB_WIDTH: usize = 4;

/// A span to underline below a source line, with its mark and its message.
struct Annotation<'a> {
    line_number: u32,
    /// The number of characters before the span on its line.
    start: usize,
    char_count: usize,
    mark: char,
    message: Option<&'a str>,
}

/// A fix applied to a copy of its file's text: the changed lines, and the new text to
/// underline with `~` when it fits on one line.
struct Correction {
    /// The corrected copy, in a source map of its own, so that `line_text` can split it into
    /// lines.
    source_map: SourceMap,
    file: FileId,
    first_line_number: u32,
    last_line_number: u32,
    new_text: Option<Annotation<'static>>,
}

impl Correction {
    /// Applies `fix` to a copy of `file`'s text.
    ///
    /// The corrected lines run from the line where the span starts to the line where the new
    /// text ends, even when the new text ends with a newline.
    fn new(file: &SourceFile, fix: &Fix) -> Self {
        let span = fix.span();
        let replacement = fix.replacement();
        // Call `line_col` first: its panic messages say more about a bad span than
        // `replace_range`'s would.
        let start_line_col = file.line_col(span.start());
        let end_line_col = file.line_col(span.end());

        let mut text = file.text().to_string();
        text.replace_range(span.start() as usize..span.end() as usize, replacement);

        let mut source_map = SourceMap::new();
        let corrected_file = source_map.add(file.name().to_string(), text);
        let new_text_end = u32::try_from(span.start() as usize + replacement.len())
            .expect("add() checked that the offsets of the corrected text fit in u32");
        let last_line_number = source_map
            .file(corrected_file)
            .line_col(new_text_end)
            .line();

        let is_on_one_line =
            start_line_col.line() == end_line_col.line() && !replacement.contains('\n');
        // A deletion leaves nothing to underline.
        let new_text = (is_on_one_line && !replacement.is_empty()).then(|| Annotation {
            line_number: start_line_col.line(),
            start: start_line_col.column() as usize - 1,
            char_count: replacement.chars().count(),
            mark: '~',
            message: None,
        });

        Self {
            source_map,
            file: corrected_file,
            first_line_number: start_line_col.line(),
            last_line_number,
            new_text,
        }
    }
}

/// Renders `diagnostic` as text for the terminal.
///
/// The output starts with `error[CODE]: title` and the location, then shows each source line
/// where a span starts, with its underlines:
///
/// - `^` marks the primary span and `-` a secondary label, each followed by its message.
/// - Lines appear in file order, and `...` stands for the lines skipped in between.
/// - Underlines on the same line are sorted by column.
///
/// The reason, help and fix come last. The fix shows the corrected lines, with the new text
/// underlined with `~` when it fits on one line. Line numbers are right-aligned to the widest
/// one shown, the fix's included.
///
/// A diagnostic about a whole file shows only the file name as its location, and only its
/// labels' lines.
///
/// Every line ends with `\n`, including the last. In every text, file names included, tabs
/// become four spaces and characters that LEX-2 forbids become `�`, so that nothing can send
/// control sequences to the terminal.
///
/// # Examples
///
/// ```
/// use frame_diagnostics::{Diagnostic, Fix, Prefix, Primary, RuleCode, render};
/// use frame_source::{SourceMap, Span};
///
/// let mut source_map = SourceMap::new();
/// let file = source_map.add(String::from("demo.fm"), String::from("if a && b {\n}\n"));
/// let diagnostic = Diagnostic::new(
///     RuleCode::new(Prefix::Lex, 23),
///     "`&&` is not an operator",
///     Primary::span_with_message(Span::new(file, 5, 7), "use `and`"),
/// )
/// .with_help("write `and` instead of `&&`")
/// .with_fix(Fix::new(Span::new(file, 5, 7), "and"));
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
///         "  = fix:\n",
///         "1 | if a and b {\n",
///         "  |      ~~~\n",
///     )
/// );
/// ```
///
/// # Panics
///
/// Panics if:
///
/// - the primary location's file is not in `source_map`;
/// - a label or the fix is in another file;
/// - a span ends past the end of its text or splits a character.
pub fn render(diagnostic: &Diagnostic, source_map: &SourceMap) -> String {
    let file_id = primary_file(diagnostic.primary());
    let file = source_map.file(file_id);
    assert_in_primary_file(diagnostic, file_id, source_map);

    let annotations = annotations(diagnostic, file);
    let correction = diagnostic.fix().map(|fix| Correction::new(file, fix));
    let margin_width = margin_width(&annotations, correction.as_ref());

    let mut lines = vec![
        title_line(diagnostic),
        location_line(&location(diagnostic.primary(), file), margin_width),
    ];
    lines.extend(excerpt_lines(file, &annotations, margin_width));
    lines.extend(end_lines(diagnostic, correction.as_ref(), margin_width));

    text_from_lines(&lines)
}

fn primary_file(primary: &Primary) -> FileId {
    match primary {
        Primary::File { file } => *file,
        Primary::Span { span, .. } => span.file(),
    }
}

/// Labels and the fix must be in the primary location's file: another file would need its
/// own location line.
// TODO(J19): revisit when a module spans several files.
fn assert_in_primary_file(diagnostic: &Diagnostic, primary_file: FileId, source_map: &SourceMap) {
    let name = |file| source_map.file(file).name();

    for label in diagnostic.labels() {
        assert!(
            label.span().file() == primary_file,
            "a label must be in the file of the primary location, '{}', not in '{}'",
            name(primary_file),
            name(label.span().file()),
        );
    }

    if let Some(fix) = diagnostic.fix() {
        assert!(
            fix.span().file() == primary_file,
            "the fix must be in the file of the primary location, '{}', not in '{}'",
            name(primary_file),
            name(fix.span().file()),
        );
    }
}

/// Returns the spans to underline, in file order.
///
/// The sort is stable: at the same position, the primary span comes first, then the labels in
/// the order they were added.
fn annotations<'a>(diagnostic: &'a Diagnostic, file: &SourceFile) -> Vec<Annotation<'a>> {
    let primary = match diagnostic.primary() {
        Primary::File { .. } => None,
        Primary::Span { span, message } => {
            Some(span_annotation(file, *span, '^', message.as_deref()))
        }
    };
    let labels = diagnostic
        .labels()
        .iter()
        .map(|label| span_annotation(file, label.span(), '-', Some(label.message())));

    let mut annotations: Vec<Annotation> = primary.into_iter().chain(labels).collect();
    annotations.sort_by_key(|annotation| (annotation.line_number, annotation.start));

    annotations
}

/// A span over several lines is only underlined on its first line, up to the line's end.
fn span_annotation<'a>(
    file: &SourceFile,
    span: Span,
    mark: char,
    message: Option<&'a str>,
) -> Annotation<'a> {
    let start_line_col = file.line_col(span.start());
    let end_line_col = file.line_col(span.end());
    let line_text = file.line_text(start_line_col.line());

    let underline_start = start_line_col.column() as usize - 1;
    let underline_char_count = if end_line_col.line() > start_line_col.line() {
        line_text.chars().count()
    } else {
        (end_line_col.column() as usize - 1) - underline_start
    };

    Annotation {
        line_number: start_line_col.line(),
        start: underline_start,
        char_count: underline_char_count,
        mark,
        message,
    }
}

/// The fix's line numbers count too, so that every margin lines up.
fn margin_width(annotations: &[Annotation], correction: Option<&Correction>) -> usize {
    annotations
        .iter()
        .map(|annotation| annotation.line_number)
        .chain(correction.map(|correction| correction.last_line_number))
        .max()
        .map_or(FILE_LEVEL_MARGIN_WIDTH, |largest_line_number| {
            let digit_count = largest_line_number.ilog10() as usize + 1;
            // Room for the line number and a space; the `|` comes after the margin.
            digit_count + 1
        })
}

/// Returns `path:line:column` for a span, or just the file name for a whole file.
fn location(primary: &Primary, file: &SourceFile) -> String {
    match primary {
        Primary::File { .. } => file.name().to_string(),
        Primary::Span { span, .. } => {
            Location::new(file.name(), file.line_col(span.start())).to_string()
        }
    }
}

fn title_line(diagnostic: &Diagnostic) -> String {
    format!(
        "error[{}]: {}",
        diagnostic.code(),
        displayed_text(diagnostic.title())
    )
}

fn location_line(location: &str, margin_width: usize) -> String {
    format!(
        "{}--> {}",
        " ".repeat(margin_width - 1),
        displayed_text(location)
    )
}

/// Returns the separator, then each line where an annotation starts, followed by its
/// underlines. `...` stands for the lines skipped in between.
fn excerpt_lines(
    file: &SourceFile,
    annotations: &[Annotation],
    margin_width: usize,
) -> Vec<String> {
    if annotations.is_empty() {
        return vec![];
    }

    let mut lines = vec![separator(margin_width)];
    let mut previous_line_number: Option<u32> = None;

    for line_annotations in annotations.chunk_by(|a, b| a.line_number == b.line_number) {
        let line_number = line_annotations[0].line_number;
        let line_text = file.line_text(line_number);

        if previous_line_number.is_some_and(|previous| line_number > previous + 1) {
            lines.push(ellipsis_line());
        }

        lines.push(source_line(line_text, line_number, margin_width));
        lines.extend(
            line_annotations
                .iter()
                .map(|annotation| underline_line(line_text, annotation, margin_width)),
        );
        previous_line_number = Some(line_number);
    }

    lines
}

/// `...` starts in the first column, with no margin.
fn ellipsis_line() -> String {
    String::from("...")
}

fn source_line(line_text: &str, line_number: u32, margin_width: usize) -> String {
    // The number is right-aligned, before the space that ends the margin.
    let line_number_width = margin_width - 1;
    let mut line = format!("{line_number:>line_number_width$} |");
    // An empty line ends at the bar, without a trailing space.
    if !line_text.is_empty() {
        line.push(' ');
        line.push_str(&displayed_text(line_text));
    }

    line
}

fn underline_line(line_text: &str, annotation: &Annotation, margin_width: usize) -> String {
    let mut line = format!(
        "{}| {}",
        " ".repeat(margin_width),
        underline(
            line_text,
            annotation.start,
            annotation.char_count,
            annotation.mark
        )
    );

    if let Some(message) = annotation.message
        && !message.is_empty()
    {
        line.push(' ');
        line.push_str(&displayed_text(message));
    }

    line
}

/// Underlines up to `char_count` characters of `line_text`, after its first `start` ones.
///
/// The marks stop at the end of the line. A tab is four columns wide, and an empty span still
/// gets one mark.
fn underline(line_text: &str, start: usize, char_count: usize, mark: char) -> String {
    let space_count = display_width(line_text, 0, start);
    let mark_count = display_width(line_text, start, char_count).max(1);
    let marks: String = iter::repeat_n(mark, mark_count).collect();

    format!("{}{marks}", " ".repeat(space_count))
}

fn end_lines(
    diagnostic: &Diagnostic,
    correction: Option<&Correction>,
    margin_width: usize,
) -> Vec<String> {
    let mut lines: Vec<String> = vec![];
    let reason = diagnostic
        .reason()
        .map(|reason| end_line(reason, "reason", margin_width));
    let help = diagnostic
        .help()
        .map(|help| end_line(help, "help", margin_width));

    if reason.is_some() || help.is_some() || correction.is_some() {
        lines.push(separator(margin_width));
    }

    if let Some(reason) = reason {
        lines.push(reason);
    }

    if let Some(help) = help {
        lines.push(help);
    }

    if let Some(correction) = correction {
        lines.extend(fix_lines(correction, margin_width));
    }

    lines
}

/// Returns the `= fix:` line, then the numbered corrected lines, without a separator.
fn fix_lines(correction: &Correction, margin_width: usize) -> Vec<String> {
    let file = correction.source_map.file(correction.file);
    let mut lines = vec![fix_heading_line(margin_width)];

    for line_number in correction.first_line_number..=correction.last_line_number {
        lines.push(source_line(
            file.line_text(line_number),
            line_number,
            margin_width,
        ));
    }

    // A new text on one line leaves a single corrected line, and the `~` go under it.
    if let Some(new_text) = &correction.new_text {
        lines.push(underline_line(
            file.line_text(new_text.line_number),
            new_text,
            margin_width,
        ));
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

fn fix_heading_line(margin_width: usize) -> String {
    format!("{}= fix:", " ".repeat(margin_width))
}

fn text_from_lines(lines: &[String]) -> String {
    let mut text = String::new();
    for line in lines {
        text.push_str(line);
        text.push('\n');
    }
    text
}

/// Tabs become four spaces, so that the marks below line up. Characters that LEX-2 forbids
/// become `�`: otherwise a `\r` or an escape sequence in the source could control the
/// terminal.
fn displayed_text(text: &str) -> String {
    let mut displayed = String::new();
    for c in text.chars() {
        match c {
            '\t' => displayed.push_str(&" ".repeat(TAB_WIDTH)),
            c if is_refused_by_lex_2(c) => {
                displayed.push(char::REPLACEMENT_CHARACTER);
            }
            c => displayed.push(c),
        }
    }

    displayed
}

fn display_width(text: &str, skip: usize, count: usize) -> usize {
    text.chars()
        .skip(skip)
        .take(count)
        .map(|c| match c {
            '\t' => TAB_WIDTH,
            _ => 1,
        })
        .sum()
}

const fn is_refused_by_lex_2(c: char) -> bool {
    c.is_control()
        || matches!(c, '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}')
}

#[cfg(test)]
mod tests;
