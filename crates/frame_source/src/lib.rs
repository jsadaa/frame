//! The `frame` source crate.

use std::fmt;

/// A source file id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FileId(u32);

/// A source file.
#[derive(Debug, Clone)]
pub struct SourceFile {
    /// The displayed name.
    name: String,
    /// The text content of the source file.
    text: String,
    /// The lines starts offsets, always contains at least one start (0), even if the text is empty.
    lines_starts: Vec<u32>,
}

impl SourceFile {
    /// Get the name of the source file.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the text of the source file.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Get the the line and column of an offset in the file.
    ///
    /// # Panics
    ///
    /// Panics if the offset in the text is in the middle of a multibyte char
    pub fn offset_line_col(&self, offset: u32) -> LineCol {
        let line_index =
            usize_to_u32(self.lines_starts.partition_point(|&start| start <= offset) - 1);
        let line_start_offset = self.lines_starts[line_index as usize];

        let column = usize_to_u32(
            self.text[line_start_offset as usize..offset as usize]
                .chars()
                .count()
                + 1,
        );

        LineCol {
            line: line_index + 1,
            column,
        }
    }

    /// Get the text content of line number `index` as a string.
    ///
    /// Remove the trailing `'\n'` or `'\r\n'` at the end of the line.
    /// Lines are indexed from 1.
    ///
    /// # Panics
    ///
    /// Panic if the line doesn't exists in the file.
    pub fn read_line(&self, line_number: u32) -> &str {
        if line_number == 0 || self.lines_count() < line_number {
            panic!("line {line_number} doesn't exists in the file");
        }

        let line_index = line_number - 1;
        let line_start_offset = *self
            .lines_starts
            .get(line_index as usize)
            .expect("at this point, we know the line number exists in the file");

        // there's always at least one line index.
        let line_end_offset = if let Some(next_line_start_offset) =
            self.lines_starts.get((line_index + 1) as usize)
        {
            *next_line_start_offset
        } else {
            usize_to_u32(self.text.len())
        };

        let line_text: &str = &self.text[line_start_offset as usize..line_end_offset as usize];

        if let Some(stripped) = line_text.strip_suffix("\r\n") {
            return stripped;
        }

        if let Some(stripped) = line_text.strip_suffix("\n") {
            return stripped;
        }

        line_text
    }

    /// Get the number of lines in the file text.
    pub fn lines_count(&self) -> u32 {
        usize_to_u32(self.lines_starts.len())
    }
}

/// The line and the column of a location in a source file text.
pub struct LineCol {
    line: u32,
    column: u32,
}

impl fmt::Display for LineCol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

/// A location in a source file text.
pub struct Location {
    name: String,
    line_col: LineCol,
}

impl Location {
    /// Create a Location from a line and column struct
    pub fn from_line_col(line_col: LineCol, name: &str) -> Self {
        Self {
            name: name.to_string(),
            line_col,
        }
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}",
            self.name, self.line_col.line, self.line_col.column
        )
    }
}

/// A collection of source files.
#[derive(Debug)]
pub struct SourceMap {
    inner: Vec<SourceFile>,
}

impl Default for SourceMap {
    fn default() -> Self {
        SourceMap::new()
    }
}

impl SourceMap {
    /// Create a new empty `SourceMap`.
    pub fn new() -> Self {
        SourceMap { inner: vec![] }
    }

    /// Add a source file and get its id.
    ///
    /// # Panics
    ///
    /// Panics if the length of the source file content exceeds the size of a u32 integer.
    pub fn add(&mut self, name: String, text: String) -> FileId {
        if text.len() <= u32::MAX as usize {
            let lines_starts = lines_starts(&text);

            self.inner.push(SourceFile {
                name,
                text,
                lines_starts,
            });

            return FileId(usize_to_u32(self.inner.len()));
        }

        panic!("the source file content length does not fit in a u32 integer");
    }

    /// Get a source file.
    ///
    /// # Panics
    ///
    /// Panics if `id` does not exists in the source map.
    pub fn file(&self, id: FileId) -> &SourceFile {
        let index = (id.0 - 1) as usize;
        if let Some(file) = self.inner.get(index) {
            return file;
        }

        panic!("the file id does not belong to the source map");
    }
}

/// A chunk of text in a `File`, from `start` to `end` (exclusive).
#[derive(Debug, PartialEq, Eq)]
pub struct Span {
    /// The source file on the file system.
    file: FileId,
    /// offset of the start in the text.
    start: u32,
    /// offset of the end in the text, exclusive.
    end: u32,
}

impl Span {
    /// Create a new span
    pub fn new(file: FileId, start: u32, end: u32) -> Self {
        Self { file, start, end }
    }

    /// Check if the span offset is 0.
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Returns the length of the span.
    pub fn len(&self) -> u32 {
        self.end - self.start
    }

    /// Return the minimum span covering `self` and `other` and everything in between.
    ///
    /// # Panics
    ///
    /// Panics if the `other` span file is different from `self`.
    pub fn minimum_span_with(&self, other: &Span) -> Span {
        if self.file != other.file {
            panic!("a span can only be produced from two span of the same file")
        }

        Self {
            file: self.file,
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

fn lines_starts(text: &str) -> Vec<u32> {
    let mut bytes = text.as_bytes().iter().enumerate().peekable();
    let mut lines_starts: Vec<u32> = vec![0];

    while let Some((i, byte)) = bytes.next() {
        let index = usize_to_u32(i);

        match byte {
            b'\n' => lines_starts.push(index + 1),
            b'\r' if bytes.peek().is_some() && *bytes.peek().unwrap().1 == b'\n' => {
                lines_starts.push(index + 2);
                bytes.next();
            }
            _ => {}
        };
    }

    lines_starts
}

fn usize_to_u32(n: usize) -> u32 {
    u32::try_from(n).expect("a u32 conversion from usize can't fail at this point as the source file length fits in a u32 integer")
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

    fn assert_position(file: &SourceFile, offset: u32, expected_position: &str) {
        assert_eq!(
            format!("{}", file.offset_line_col(offset)),
            expected_position
        );
    }

    fn assert_line(file: &SourceFile, line_number: u32, expected_line_text: &str) {
        assert_eq!(file.read_line(line_number), expected_line_text);
    }

    fn assert_lines_count(file: &SourceFile, expected_lines_count: u32) {
        assert_eq!(file.lines_count(), expected_lines_count);
    }

    #[test]
    fn start_of_file() {
        let file = file_of("abc");
        assert_position(&file, 0, "1:1");
        assert_line(&file, 1, "abc");
        assert_lines_count(&file, 1);
    }

    #[test]
    fn empty_file() {
        let file = file_of("");
        assert_position(&file, 0, "1:1");
        assert_line(&file, 1, "");
        assert_lines_count(&file, 1);
    }

    #[test]
    fn after_line_jump() {
        let file = file_of("ab\ncd");
        assert_position(&file, 3, "2:1");
        assert_line(&file, 1, "ab");
        assert_lines_count(&file, 2);
    }

    #[test]
    fn two_bytes_character() {
        let file = file_of("let a = 1\nlet é = 2\n");
        assert_position(&file, 17, "2:7");
        assert_line(&file, 2, "let é = 2");
        assert_lines_count(&file, 3);
    }

    #[test]
    fn four_bytes_character() {
        let file = file_of("a😀b");
        assert_position(&file, 5, "1:3");
        assert_line(&file, 1, "a😀b");
        assert_lines_count(&file, 1);
    }

    #[test]
    fn tabulation() {
        let file = file_of("\tx");
        assert_position(&file, 1, "1:2");
        assert_line(&file, 1, "\tx");
        assert_lines_count(&file, 1);
    }

    #[test]
    fn end_of_line_with_carriage_return() {
        let file = file_of("a\r\nb");
        assert_position(&file, 3, "2:1");
        assert_line(&file, 1, "a");
        assert_lines_count(&file, 2);
    }

    #[test]
    fn on_carriage_return() {
        let file = file_of("a\r\nb");
        assert_position(&file, 1, "1:2");
        assert_line(&file, 2, "b");
        assert_lines_count(&file, 2);
    }

    #[test]
    fn carriage_return_alone() {
        let file = file_of("a\r");
        assert_position(&file, 0, "1:1");
        assert_line(&file, 1, "a\r");
        assert_lines_count(&file, 1);
    }

    #[test]
    fn end_of_file_after_line_jump() {
        let file = file_of("ab\n");
        assert_position(&file, 3, "2:1");
        assert_line(&file, 1, "ab");
        assert_lines_count(&file, 2);
    }

    #[test]
    fn end_of_file_without_line_jump() {
        let file = file_of("ab");
        assert_position(&file, 2, "1:3");
        assert_line(&file, 1, "ab");
        assert_lines_count(&file, 1);
    }

    #[test]
    fn distincts_file_ids() {
        let mut source_map = SourceMap::new();
        let id_a = source_map.add("a.fm".to_string(), "".to_string());
        let id_b = source_map.add("b.fm".to_string(), "".to_string());
        assert_ne!(id_a, id_b);
    }

    #[test]
    #[should_panic(expected = "end byte index 3 is out of bounds for string of length 2")]
    fn offset_out_of_text() {
        let file = file_of("ab");
        assert_position(&file, 3, "whatever");
    }

    #[test]
    #[should_panic(
        expected = "end byte index 1 is not a char boundary; it is inside 'é' (bytes 0..2 of string)"
    )]
    fn offset_in_the_middle_of_a_char() {
        let file = file_of("é");
        assert_position(&file, 1, "whatever");
    }

    #[test]
    #[should_panic(expected = "line 0 doesn't exists in the file")]
    fn line_0_out_of_file() {
        let file = file_of("ab");
        assert_line(&file, 0, "whatever");
    }

    #[test]
    #[should_panic(expected = "line 2 doesn't exists in the file")]
    fn line_out_of_file() {
        let file = file_of("ab");
        assert_line(&file, 2, "whatever");
    }

    #[test]
    fn span_length() {
        let cases = [
            (Span::new(FileId(1), 2, 5), 3, false),
            (Span::new(FileId(1), 4, 4), 0, true),
        ];

        for (span, expected_length, is_empty) in cases {
            assert_eq!(span.len(), expected_length);
            assert_eq!(span.is_empty(), is_empty);
        }
    }

    #[test]
    fn minimum_span() {
        let cases = [
            (Span::new(FileId(1), 2, 5), Span::new(FileId(1), 7, 9)),
            (Span::new(FileId(1), 7, 9), Span::new(FileId(1), 2, 5)),
        ];

        for (span_from, span_with) in cases {
            assert_eq!(
                span_from.minimum_span_with(&span_with),
                Span::new(FileId(1), 2, 9)
            );
        }
    }

    #[test]
    #[should_panic(expected = "a span can only be produced from two span of the same file")]
    fn minimum_span_from_different_file() {
        Span::new(FileId(1), 2, 5).minimum_span_with(&Span::new(FileId(2), 7, 9));
    }

    #[test]
    fn display_location() {
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

        for (location, expected_string) in cases {
            assert_eq!(format!("{location}"), expected_string.to_string())
        }
    }
}
