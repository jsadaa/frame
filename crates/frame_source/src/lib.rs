//! Source files, and positions in their text.
//!
//! A [`SourceMap`] holds the files; a [`Span`] designates a piece of a file's text by byte
//! offsets. Lines and columns ([`LineCol`]) are computed from an offset, by the
//! [`SourceFile`], only to show a position (DIA-5).

use std::fmt;

/// The id of a file in a [`SourceMap`]; only [`SourceMap::add`] makes one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FileId(u32);

/// A file of a [`SourceMap`]: its name, its text, and where its lines start.
#[derive(Debug, Clone)]
pub struct SourceFile {
    /// The name shown in locations, such as `src/main.fm`.
    name: String,
    text: String,
    /// The byte offset where each line starts, in order; the first is 0, even for an empty
    /// text.
    line_starts: Vec<u32>,
}

impl SourceFile {
    /// Returns the name shown in locations.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the line and the column of a byte offset, both counted from 1, the column in
    /// characters (DIA-5). The offset of the end of the text is valid.
    ///
    /// # Panics
    ///
    /// Panics if `offset` is past the end of the text, or inside a character.
    pub fn line_col(&self, offset: u32) -> LineCol {
        assert!(
            offset as usize <= self.text.len(),
            "offset {offset} is out of range: the text length is {}",
            self.text.len()
        );
        assert!(
            self.text.is_char_boundary(offset as usize),
            "offset {offset} is not a char boundary",
        );

        let line_index = self.line_starts.partition_point(|&start| start <= offset) - 1;
        let line_start = self.line_starts[line_index];

        let column = usize_to_u32(
            self.text[line_start as usize..offset as usize]
                .chars()
                .count()
                + 1,
        );

        LineCol {
            line: usize_to_u32(line_index + 1),
            column,
        }
    }

    /// Returns the text of a line, counted from 1, without its line ending: `\n` or `\r\n`
    /// (LEX-3). A lone `\r` stays in the text.
    ///
    /// # Panics
    ///
    /// Panics if the file has no line `line_number`.
    pub fn line_text(&self, line_number: u32) -> &str {
        assert!(
            line_number > 0 && line_number <= self.line_count(),
            "line {line_number} is out of range: the line count is {}",
            self.line_count()
        );

        let line_index = line_number - 1;
        let line_start = self.line_starts[line_index as usize];

        // The last line has no next start: it ends with the text.
        let line_end = self
            .line_starts
            .get((line_index + 1) as usize)
            .copied()
            .unwrap_or(usize_to_u32(self.text.len()));

        let line: &str = &self.text[line_start as usize..line_end as usize];

        if let Some(stripped) = line.strip_suffix("\r\n") {
            return stripped;
        }

        if let Some(stripped) = line.strip_suffix("\n") {
            return stripped;
        }

        line
    }

    /// Returns the number of lines: one more than the number of line endings, so that an
    /// empty text has one line.
    pub fn line_count(&self) -> u32 {
        usize_to_u32(self.line_starts.len())
    }
}

/// A line and a column, both counted from 1; the column counts characters, a tab as one
/// (DIA-5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineCol {
    line: u32,
    column: u32,
}

impl LineCol {
    /// Returns the line, counted from 1.
    pub fn line(&self) -> u32 {
        self.line
    }

    /// Returns the column, counted from 1.
    pub fn column(&self) -> u32 {
        self.column
    }
}

impl fmt::Display for LineCol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

/// A position as diagnostics show it: `path:line:column` (DIA-5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    name: String,
    line_col: LineCol,
}

impl Location {
    /// Creates the location of `line_col` in the file named `name`.
    pub fn from_line_col(line_col: LineCol, name: &str) -> Self {
        Self {
            name: name.to_string(),
            line_col,
        }
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.name, self.line_col)
    }
}

/// The source files of a compilation, each designated by its [`FileId`].
#[derive(Debug)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

impl Default for SourceMap {
    fn default() -> Self {
        SourceMap::new()
    }
}

impl SourceMap {
    /// Creates an empty source map.
    pub fn new() -> Self {
        SourceMap { files: vec![] }
    }

    /// Adds a file, and returns its id. The starts of its lines are computed here, once.
    ///
    /// # Panics
    ///
    /// Panics if the text has `u32::MAX` bytes or more: its offsets, and the column of its
    /// end, must fit in a `u32`.
    pub fn add(&mut self, name: String, text: String) -> FileId {
        assert!(
            text.len() < u32::MAX as usize,
            "the text of '{name}' is too long for 32-bit offsets: {} bytes",
            text.len(),
        );

        let line_starts = compute_line_starts(&text);
        let id = FileId(u32::try_from(self.files.len()).expect("the id should fit in u32"));

        self.files.push(SourceFile {
            name,
            text,
            line_starts,
        });

        id
    }

    /// Returns the file of `id`.
    ///
    /// # Panics
    ///
    /// Panics if no file of this source map has this `id`: it comes from another one.
    pub fn file(&self, id: FileId) -> &SourceFile {
        let Some(file) = self.files.get(id.0 as usize) else {
            panic!("{id:?} is unknown to this source map");
        };

        file
    }
}

/// A piece of the text of a file, between two byte offsets: `start` is included, `end`
/// excluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    file: FileId,
    start: u32,
    end: u32,
}

impl Span {
    /// Creates the span of `start..end` in `file`.
    ///
    /// # Panics
    ///
    /// Panics if `start` is after `end`.
    pub fn new(file: FileId, start: u32, end: u32) -> Self {
        assert!(end >= start, "span start {start} is after its end {end}");
        Self { file, start, end }
    }

    /// Returns `true` if the span is empty: it marks a point, such as an insertion.
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Returns the length, in bytes.
    pub fn len(&self) -> u32 {
        self.end - self.start
    }

    /// Returns the smallest span that covers `self` and `other`, and what lies between.
    ///
    /// # Panics
    ///
    /// Panics if the two spans are in different files.
    pub fn cover(&self, other: &Span) -> Span {
        assert!(
            self.file == other.file,
            "cannot cover spans of two files: {self:?} and {other:?}"
        );

        Self {
            file: self.file,
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    /// Returns the file.
    pub fn file(&self) -> FileId {
        self.file
    }

    /// Returns the offset of the start, included.
    pub fn start(&self) -> u32 {
        self.start
    }

    /// Returns the offset of the end, excluded.
    pub fn end(&self) -> u32 {
        self.end
    }
}

/// Returns the byte offset where each line starts: 0, then the offset after each `\n`.
fn compute_line_starts(text: &str) -> Vec<u32> {
    let mut starts: Vec<u32> = vec![0];

    for (i, byte) in text.as_bytes().iter().enumerate() {
        if *byte == b'\n' {
            starts.push(usize_to_u32(i) + 1);
        }
    }

    starts
}

/// Converts a value bounded by the length of a text, which `SourceMap::add` checked to fit
/// in a `u32`.
fn usize_to_u32(n: usize) -> u32 {
    u32::try_from(n).expect("the value should fit in u32, as add() checked the text length")
}

#[cfg(test)]
mod tests {
    use super::{FileId, LineCol, Location, SourceFile, SourceMap, Span};

    fn file_of(text: &str) -> SourceFile {
        let mut source_map = SourceMap::new();
        let id = source_map.add("a.fm".to_string(), text.to_string());
        let file = source_map.file(id);
        file.clone()
    }

    fn assert_line_col(file: &SourceFile, offset: u32, expected: LineCol) {
        assert_eq!(file.line_col(offset), expected, "offset {offset}");
    }

    fn assert_line_text(file: &SourceFile, line_number: u32, expected: &str) {
        assert_eq!(file.line_text(line_number), expected, "line {line_number}");
    }

    fn assert_line_count(file: &SourceFile, expected: u32) {
        assert_eq!(file.line_count(), expected);
    }

    /// Counts the line and the column of `offset` character by character, from the start
    /// of the text: slow, but obviously right.
    fn naive_line_col(text: &str, offset: usize) -> LineCol {
        let mut line = 1;
        let mut column = 1;
        for (index, character) in text.char_indices() {
            if index == offset {
                break;
            }
            if character == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }
        LineCol { line, column }
    }

    #[test]
    fn start_of_text_is_line_1_column_1() {
        let file = file_of("abc");
        assert_line_col(&file, 0, LineCol { line: 1, column: 1 });
        assert_line_text(&file, 1, "abc");
        assert_line_count(&file, 1);
    }

    #[test]
    fn empty_text_has_one_empty_line() {
        let file = file_of("");
        assert_line_col(&file, 0, LineCol { line: 1, column: 1 });
        assert_line_text(&file, 1, "");
        assert_line_count(&file, 1);
    }

    #[test]
    fn newline_starts_the_next_line() {
        let file = file_of("ab\ncd");
        assert_line_col(&file, 3, LineCol { line: 2, column: 1 });
        assert_line_text(&file, 1, "ab");
        assert_line_count(&file, 2);
    }

    #[test]
    fn two_byte_character_is_one_column() {
        let file = file_of("let a = 1\nlet é = 2\n");
        assert_line_col(&file, 17, LineCol { line: 2, column: 7 });
        assert_line_text(&file, 2, "let é = 2");
        assert_line_count(&file, 3);
    }

    #[test]
    fn four_byte_character_is_one_column() {
        let file = file_of("a😀b");
        assert_line_col(&file, 5, LineCol { line: 1, column: 3 });
        assert_line_text(&file, 1, "a😀b");
        assert_line_count(&file, 1);
    }

    #[test]
    fn tab_is_one_column() {
        let file = file_of("\tx");
        assert_line_col(&file, 1, LineCol { line: 1, column: 2 });
        assert_line_text(&file, 1, "\tx");
        assert_line_count(&file, 1);
    }

    #[test]
    fn crlf_ends_a_line_without_its_cr() {
        let file = file_of("a\r\nb");
        assert_line_col(&file, 3, LineCol { line: 2, column: 1 });
        assert_line_text(&file, 1, "a");
        assert_line_count(&file, 2);
    }

    #[test]
    fn cr_of_a_crlf_stays_on_its_line() {
        let file = file_of("a\r\nb");
        assert_line_col(&file, 1, LineCol { line: 1, column: 2 });
        assert_line_text(&file, 2, "b");
        assert_line_count(&file, 2);
    }

    #[test]
    fn lone_cr_does_not_end_a_line() {
        let file = file_of("a\r");
        assert_line_col(&file, 0, LineCol { line: 1, column: 1 });
        assert_line_text(&file, 1, "a\r");
        assert_line_count(&file, 1);
    }

    #[test]
    fn end_of_text_after_a_newline_starts_an_empty_line() {
        let file = file_of("ab\n");
        assert_line_col(&file, 3, LineCol { line: 2, column: 1 });
        assert_line_text(&file, 1, "ab");
        assert_line_text(&file, 2, "");
        assert_line_count(&file, 2);
    }

    #[test]
    fn end_of_text_without_a_newline_follows_the_last_character() {
        let file = file_of("ab");
        assert_line_col(&file, 2, LineCol { line: 1, column: 3 });
        assert_line_text(&file, 1, "ab");
        assert_line_count(&file, 1);
    }

    #[test]
    fn added_files_get_distinct_ids() {
        let mut source_map = SourceMap::new();
        let id_a = source_map.add("a.fm".to_string(), "ab".to_string());
        let id_b = source_map.add("b.fm".to_string(), "cd".to_string());
        assert_ne!(id_a, id_b);
    }

    #[test]
    fn file_returns_the_name_and_text_added_with_its_id() {
        let mut source_map = SourceMap::new();
        let id_a = source_map.add("a.fm".to_string(), "ab".to_string());
        let id_b = source_map.add("b.fm".to_string(), "cd".to_string());

        let file_a = source_map.file(id_a);
        assert_eq!(file_a.name(), "a.fm");
        assert_eq!(file_a.text(), "ab");

        let file_b = source_map.file(id_b);
        assert_eq!(file_b.name(), "b.fm");
        assert_eq!(file_b.text(), "cd");
    }

    #[test]
    fn line_col_agrees_with_a_naive_count_at_every_offset() {
        let text = concat!(
            "ab\r\n",    // ASCII, CRLF
            "\r\n",      // an empty line, CRLF
            "\té = ü\n", // a tab, two-byte characters
            "\n",        // an empty line, LF
            "😀 x\ry\n", // a four-byte character, a lone CR
            "中文",      // three-byte characters, no final newline
        );
        let file = file_of(text);
        let offsets = text.char_indices().map(|(offset, _)| offset);

        for offset in offsets.chain([text.len()]) {
            let line_col = file.line_col(
                u32::try_from(offset).expect("the offsets of the test text should fit in u32"),
            );
            assert_eq!(line_col, naive_line_col(text, offset), "offset {offset}");
        }
    }

    #[test]
    #[should_panic(expected = "offset 3 is out of range: the text length is 2")]
    fn line_col_panics_past_the_end_of_the_text() {
        let file = file_of("ab");
        file.line_col(3);
    }

    #[test]
    #[should_panic(expected = "offset 1 is not a char boundary")]
    fn line_col_panics_inside_a_character() {
        let file = file_of("é");
        file.line_col(1);
    }

    #[test]
    #[should_panic(expected = "line 0 is out of range: the line count is 1")]
    fn line_text_panics_on_line_0() {
        let file = file_of("ab");
        file.line_text(0);
    }

    #[test]
    #[should_panic(expected = "line 2 is out of range: the line count is 1")]
    fn line_text_panics_past_the_last_line() {
        let file = file_of("ab");
        file.line_text(2);
    }

    #[test]
    fn span_len_and_is_empty() {
        let cases = [
            (Span::new(FileId(1), 2, 5), 3, false),
            (Span::new(FileId(1), 4, 4), 0, true),
        ];

        for (span, expected_len, expected_is_empty) in cases {
            assert_eq!(span.len(), expected_len, "{span:?}");
            assert_eq!(span.is_empty(), expected_is_empty, "{span:?}");
        }
    }

    #[test]
    #[should_panic(expected = "FileId(1) is unknown to this source map")]
    fn add_panics_with_an_unkown_file_id() {
        let source_map = SourceMap::new();
        source_map.file(FileId(1));
    }

    #[test]
    #[should_panic(expected = "span start 5 is after its end 2")]
    fn span_new_panics_if_start_is_after_end() {
        Span::new(FileId(1), 5, 2);
    }

    #[test]
    fn cover_spans_both_in_either_order() {
        let cases = [
            (Span::new(FileId(1), 2, 5), Span::new(FileId(1), 7, 9)),
            (Span::new(FileId(1), 7, 9), Span::new(FileId(1), 2, 5)),
        ];

        for (span, other) in cases {
            assert_eq!(
                span.cover(&other),
                Span::new(FileId(1), 2, 9),
                "{span:?} covering {other:?}"
            );
        }
    }

    #[test]
    #[should_panic(
        expected = "cannot cover spans of two files: Span { file: FileId(1), start: 2, end: 5 } and Span { file: FileId(2), start: 7, end: 9 }"
    )]
    fn cover_panics_on_spans_of_two_files() {
        Span::new(FileId(1), 2, 5).cover(&Span::new(FileId(2), 7, 9));
    }

    #[test]
    fn location_displays_path_line_column() {
        let cases = [
            (
                Location::from_line_col(LineCol { line: 2, column: 3 }, "src/main.fm"),
                "src/main.fm:2:3",
            ),
            (
                Location::from_line_col(
                    LineCol {
                        line: 1,
                        column: 10,
                    },
                    "lib.fm",
                ),
                "lib.fm:1:10",
            ),
        ];

        for (location, expected) in cases {
            assert_eq!(format!("{location}"), expected.to_string(), "{location:?}");
        }
    }
}
