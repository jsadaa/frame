//! Loading source files from disk.
//!
//! A file is read, checked against the size limit and decoded as UTF-8, then added to a
//! [`SourceMap`].

use std::{
    error::Error,
    fmt::Display,
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
};

use super::{FileId, SourceMap, Span, usize_to_u32};

/// The largest source file accepted, in bytes.
///
/// This is 16 MiB, the minimum that CNF-6 requires an implementation to accept.
pub const MAX_SOURCE_SIZE: u64 = 16 * 1024 * 1024;

/// Why a source file could not be loaded.
#[derive(Debug)]
pub enum LoadError {
    /// The file could not be opened or read. It is not added to the source map.
    Read {
        /// The path, as given to [`load_file`].
        path: PathBuf,
        /// The underlying I/O error.
        system_error: io::Error,
    },
    /// The file is not valid UTF-8.
    ///
    /// It is still added, with each invalid sequence replaced by U+FFFD, so that a
    /// diagnostic can show it.
    InvalidUtf8 {
        /// The first U+FFFD in the added text, where the first invalid byte was.
        span: Span,
    },
    /// The file is larger than the limit.
    ///
    /// It is still added, with an empty text, so that a diagnostic can name it.
    TooLarge {
        /// The id of the added file.
        file: FileId,
        /// The limit in bytes.
        limit: u64,
    },
}

impl Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Read { path, system_error } => {
                write!(f, "cannot read '{}': {system_error}", path.display())
            }
            LoadError::InvalidUtf8 { .. } => write!(f, "source file is not valid UTF-8"),
            LoadError::TooLarge { limit, .. } => {
                write!(f, "source file is too large: the limit is {limit} bytes")
            }
        }
    }
}

impl Error for LoadError {}

/// Reads the file at `path`, adds it to `source_map` under `name`, and returns its id.
///
/// A leading byte order mark is removed (LEX-1).
///
/// # Errors
///
/// Returns an error if the file cannot be read, is not valid UTF-8, or is larger than
/// [`MAX_SOURCE_SIZE`]. In the last two cases, the file is still added, so that a
/// diagnostic can point to it.
pub fn load_file(source_map: &mut SourceMap, path: &Path, name: &str) -> Result<FileId, LoadError> {
    load_file_with_limit(source_map, path, name, MAX_SOURCE_SIZE)
}

/// Same as [`load_file`], with the size limit as a parameter so that tests can use a tiny
/// one.
fn load_file_with_limit(
    source_map: &mut SourceMap,
    path: &Path,
    name: &str,
    limit: u64,
) -> Result<FileId, LoadError> {
    let bytes = read_bounded(path, limit).map_err(|error| LoadError::Read {
        path: path.to_path_buf(),
        system_error: error,
    })?;

    if bytes.len() as u64 > limit {
        return Err(LoadError::TooLarge {
            file: source_map.add(name.to_string(), String::new()),
            limit,
        });
    }

    match decode(bytes) {
        Ok(text) => Ok(source_map.add(name.to_string(), text)),
        Err((invalid_offset, lossy_text)) => {
            let end = invalid_offset + char::REPLACEMENT_CHARACTER.len_utf8();
            let file = source_map.add(name.to_string(), lossy_text);

            // The span is within the text just added, whose offsets `add` checked.
            Err(LoadError::InvalidUtf8 {
                span: Span::new(file, usize_to_u32(invalid_offset), usize_to_u32(end)),
            })
        }
    }
}

/// Reads the file at `path`, up to one byte past `limit`.
fn read_bounded(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();

    // Reading one byte past the limit tells a file at the limit from a larger one, without
    // reading all of a large file.
    File::open(path)?.take(limit + 1).read_to_end(&mut bytes)?;

    Ok(bytes)
}

/// Strips a leading byte order mark, then decodes the rest as UTF-8.
///
/// On failure, returns the offset of the first invalid byte (not counting the mark) and the
/// text with each invalid sequence replaced by U+FFFD.
fn decode(mut bytes: Vec<u8>) -> Result<String, (usize, String)> {
    // A mark anywhere else stays, for the lexer to reject (LEX-2).
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        bytes.drain(..3);
    }

    String::from_utf8(bytes).map_err(|error| {
        (
            error.utf8_error().valid_up_to(),
            String::from_utf8_lossy(&error.into_bytes()).into_owned(),
        )
    })
}

#[cfg(test)]
mod tests {
    use crate::Span;

    use super::{FileId, LoadError, SourceMap, decode, load_file_with_limit};
    use std::{
        io,
        path::{Path, PathBuf},
    };

    fn fixture_path(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/{name}"))
    }

    fn str_to_bytes(text: &str) -> Vec<u8> {
        String::from(text).into_bytes()
    }

    #[test]
    fn decode_keeps_ascii_text() {
        assert_eq!(
            decode(str_to_bytes("let x = 1")).unwrap(),
            String::from("let x = 1")
        );
    }

    #[test]
    fn decode_keeps_multibyte_characters() {
        assert_eq!(decode(str_to_bytes("é€😀")).unwrap(), String::from("é€😀"));
    }

    #[test]
    fn decode_removes_a_leading_bom() {
        assert_eq!(
            decode([0xEF, 0xBB, 0xBF, b'a'].to_vec()).unwrap(),
            String::from("a")
        );
    }

    #[test]
    fn decode_keeps_a_bom_anywhere_else() {
        assert_eq!(
            decode([b'a', 0xEF, 0xBB, 0xBF, b'b'].to_vec()).unwrap(),
            String::from("a\u{feff}b")
        );
    }

    #[test]
    fn decode_removes_a_bom_alone() {
        assert_eq!(decode([0xEF, 0xBB, 0xBF].to_vec()).unwrap(), String::new());
    }

    #[test]
    fn decode_rejects_a_lone_continuation_byte() {
        assert_eq!(
            decode([0x80].to_vec()).unwrap_err(),
            (0, String::from("\u{FFFD}"))
        );
    }

    #[test]
    fn decode_rejects_an_invalid_byte_in_the_middle() {
        assert_eq!(
            decode([b'a', b'b', 0xFF, b'c'].to_vec()).unwrap_err(),
            (2, String::from("ab\u{FFFD}c"))
        );
    }

    #[test]
    fn decode_rejects_a_sequence_truncated_at_the_end() {
        assert_eq!(
            decode([b'a', 0xC3].to_vec()).unwrap_err(),
            (1, String::from("a\u{FFFD}"))
        );
    }

    #[test]
    fn decode_rejects_an_overlong_encoding() {
        assert_eq!(
            decode([0xC0, 0x80].to_vec()).unwrap_err(),
            (0, String::from("\u{FFFD}\u{FFFD}"))
        );
    }

    #[test]
    fn decode_rejects_a_surrogate() {
        assert_eq!(
            decode([0xED, 0xA0, 0x80].to_vec()).unwrap_err(),
            (0, String::from("\u{FFFD}\u{FFFD}\u{FFFD}"))
        );
    }

    #[test]
    fn decode_rejects_a_code_point_beyond_u10ffff() {
        assert_eq!(
            decode([0xF4, 0x90, 0x80, 0x80].to_vec()).unwrap_err(),
            (0, String::from("\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}"))
        );
    }

    #[test]
    fn decode_counts_the_invalid_offset_without_the_bom() {
        assert_eq!(
            decode([0xEF, 0xBB, 0xBF, 0xFF].to_vec()).unwrap_err(),
            (0, String::from("\u{FFFD}"))
        );
    }

    #[test]
    fn load_accepts_a_file_at_the_limit() {
        let path = fixture_path("valid.fm");
        let source_map = &mut SourceMap::new();
        let id = load_file_with_limit(source_map, &path, "valid.fm", 10).unwrap();
        let file = source_map.file(id);
        assert_eq!(id, FileId(0));
        assert_eq!(file.text(), "let a = 1\n");
    }

    #[test]
    fn load_rejects_a_file_over_the_limit() {
        let path = fixture_path("valid.fm");
        let source_map = &mut SourceMap::new();
        let result = load_file_with_limit(source_map, &path, "valid.fm", 9);
        let Err(LoadError::TooLarge { file: id, limit }) = result else {
            panic!("expected a TooLarge error, got {result:?}")
        };
        let file = source_map.file(id);

        assert_eq!(id, FileId(0));
        assert_eq!(limit, 9);
        assert_eq!(file.name(), "valid.fm");
        assert_eq!(file.text(), "");
    }

    #[test]
    fn read_error_displays_the_path_and_the_system_error() {
        let error = LoadError::Read {
            path: PathBuf::from("src/a.fm"),
            system_error: io::Error::from(io::ErrorKind::NotFound),
        };
        assert_eq!(
            format!("{error}"),
            String::from("cannot read 'src/a.fm': entity not found")
        );
    }

    #[test]
    fn invalid_utf8_error_displays_its_title() {
        let error = LoadError::InvalidUtf8 {
            span: Span::new(FileId(0), 4, 9),
        };
        assert_eq!(
            format!("{error}"),
            String::from("source file is not valid UTF-8")
        );
    }

    #[test]
    fn too_large_error_displays_the_limit() {
        let error = LoadError::TooLarge {
            file: FileId(0),
            limit: 10,
        };
        assert_eq!(
            format!("{error}"),
            String::from("source file is too large: the limit is 10 bytes")
        );
    }
}
