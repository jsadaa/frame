//! Loading a source file: read from the disk, checked as UTF-8 (LEX-1) and against the
//! size limit (CNF-6), then added to a [`SourceMap`].

use std::{
    error::Error,
    fmt::Display,
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
};

use super::{FileId, SourceMap, Span};

/// The size limit of a source file, in bytes: 16 MiB, the least that CNF-6 lets an
/// implementation accept.
pub const MAX_SOURCE_SIZE: u64 = 16 * 1024 * 1024;

/// Why a source file could not be loaded.
#[derive(Debug)]
pub enum LoadError {
    /// The system could not open or read the file. It is not added to the source map.
    Read {
        /// The path, as given to [`load_file`].
        path: PathBuf,
        /// The system error.
        system_error: io::Error,
    },
    /// The file is not valid UTF-8 (LEX-1). It is added with each invalid sequence replaced
    /// by U+FFFD;
    InvalidUtf8 {
        /// The span, which covers the first invalid sequence.
        span: Span,
    },
    /// The file is larger than the limit (CNF-6). It is added with an empty text, so that a
    /// diagnostic can name it.
    TooLarge {
        /// The id of the file added.
        file: FileId,
        /// The limit, in bytes.
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
            LoadError::TooLarge { file: _, limit } => {
                write!(f, "source file is too large: the limit is {limit} bytes")
            }
        }
    }
}

impl Error for LoadError {}

/// Reads the file at `path`, adds it to `source_map` under `name`, and returns its id. A
/// leading byte order mark is removed (LEX-1).
///
/// # Errors
///
/// Returns a [`LoadError`] if the file cannot be read, is not valid UTF-8, or is larger than
/// [`MAX_SOURCE_SIZE`]. In the last two cases, the file is added anyway, for the diagnostic.
pub fn load_file(source_map: &mut SourceMap, path: &Path, name: &str) -> Result<FileId, LoadError> {
    load_file_with_limit(source_map, path, name, MAX_SOURCE_SIZE)
}

/// [`load_file`], with the size limit as a parameter, so that tests can use a few bytes.
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
            let invalid_offset = usize_to_u32(
                invalid_offset,
                "the bytes are within the size limit, so their offsets fit in u32",
            );

            Err(LoadError::InvalidUtf8 {
                span: Span::new(
                    source_map.add(name.to_string(), lossy_text),
                    invalid_offset,
                    invalid_offset
                        + usize_to_u32(
                            char::REPLACEMENT_CHARACTER.len_utf8(),
                            "U+FFFD takes 3 bytes",
                        ),
                ),
            })
        }
    }
}

/// Reads the file at `path`, up to one byte past `limit`.
fn read_bounded(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();

    // One byte past the limit tells a file at the limit from a larger one, without reading
    // the larger one whole.
    File::open(path)?.take(limit + 1).read_to_end(&mut bytes)?;

    Ok(bytes)
}

/// Decodes `bytes` as UTF-8, without a leading byte order mark (LEX-1).
///
/// # Errors
///
/// If the bytes are not valid UTF-8, returns the offset of the first invalid byte, counted
/// without the mark, and the text with each invalid sequence replaced by U+FFFD.
fn decode(mut bytes: Vec<u8>) -> Result<String, (usize, String)> {
    // A mark elsewhere stays: the lexer refuses it (LEX-2).
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

fn usize_to_u32(num: usize, message: &str) -> u32 {
    u32::try_from(num).expect(message)
}

#[cfg(test)]
mod tests {
    use crate::Span;

    use super::{FileId, LoadError, SourceMap, decode, load_file_with_limit};
    use std::{
        fs, io,
        path::{Path, PathBuf},
    };

    fn fixture_path(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/{}", name))
    }

    fn str_to_bytes(text: &str) -> Vec<u8> {
        String::from(text).into_bytes()
    }

    fn bytes_to_lossy_string(bytes: Vec<u8>) -> String {
        String::from_utf8_lossy(&bytes).to_string()
    }

    #[test]
    fn decode_keeps_ascii_text() {
        assert_eq!(
            decode(str_to_bytes("let x = 1")).unwrap(),
            String::from("let x = 1")
        )
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
        )
    }

    #[test]
    fn decode_keeps_a_bom_anywhere_else() {
        assert_eq!(
            decode([b'a', 0xEF, 0xBB, 0xBF, b'b'].to_vec()).unwrap(),
            String::from("a\u{feff}b")
        )
    }

    #[test]
    fn decode_removes_a_bom_alone() {
        assert_eq!(
            decode([0xEF, 0xBB, 0xBF].to_vec()).unwrap(),
            String::from("")
        )
    }

    #[test]
    fn decode_rejects_a_lone_continuation_byte() {
        let bytes: Vec<u8> = [0x80].to_vec();
        assert_eq!(
            decode(bytes.clone()).unwrap_err(),
            (0, bytes_to_lossy_string(bytes))
        );
    }

    #[test]
    fn decode_rejects_an_invalid_byte_in_the_middle() {
        let bytes: Vec<u8> = [b'a', b'b', 0xFF, b'c'].to_vec();
        assert_eq!(
            decode(bytes.clone()).unwrap_err(),
            (2, bytes_to_lossy_string(bytes))
        );
    }

    #[test]
    fn decode_rejects_a_sequence_truncated_at_the_end() {
        let bytes: Vec<u8> = [b'a', 0xC3].to_vec();
        assert_eq!(
            decode(bytes.clone()).unwrap_err(),
            (1, bytes_to_lossy_string(bytes))
        );
    }

    #[test]
    fn decode_rejects_an_overlong_encoding() {
        let bytes: Vec<u8> = [0xC0, 0x80].to_vec();
        assert_eq!(
            decode(bytes.clone()).unwrap_err(),
            (0, bytes_to_lossy_string(bytes))
        );
    }

    #[test]
    fn decode_rejects_a_surrogate() {
        let bytes: Vec<u8> = [0xED, 0xA0, 0x80].to_vec();
        assert_eq!(
            decode(bytes.clone()).unwrap_err(),
            (0, bytes_to_lossy_string(bytes))
        );
    }

    #[test]
    fn decode_rejects_a_code_point_beyond_u10ffff() {
        let bytes: Vec<u8> = [0xF4, 0x90, 0x80, 0x80].to_vec();
        assert_eq!(
            decode(bytes.clone()).unwrap_err(),
            (0, bytes_to_lossy_string(bytes))
        );
    }

    #[test]
    fn decode_counts_the_invalid_offset_without_the_bom() {
        let mut bytes: Vec<u8> = [0xEF, 0xBB, 0xBF, 0xFF].to_vec();
        assert_eq!(
            decode(bytes.clone()).unwrap_err(),
            (0, bytes_to_lossy_string(bytes.drain(3..).collect()))
        );
    }

    #[test]
    fn load_accepts_a_file_at_the_limit() {
        let path = fixture_path("valid.fm");
        let source_map = &mut SourceMap::new();
        let id = load_file_with_limit(source_map, &path, "valid.fm", 10).unwrap();
        let file = source_map.file(id);
        assert_eq!(id, FileId(0));
        assert_eq!(file.text(), fs::read_to_string(path).unwrap())
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
        )
    }

    #[test]
    fn invalid_utf8_error_displays_its_title() {
        let error = LoadError::InvalidUtf8 {
            span: Span::new(FileId(0), 4, 9),
        };
        assert_eq!(
            format!("{error}"),
            String::from("source file is not valid UTF-8")
        )
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
        )
    }
}
