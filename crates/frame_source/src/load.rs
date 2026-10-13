//! Validate and load a source file into the source map

use std::{error::Error, fmt::Display, fs::File, io::Read, path::Path};

use super::{FileId, SourceMap, Span};

/// The maximum size allowed for a frame source file
pub const MAX_SOURCE_SIZE: u64 = 16 * 1024 * 1024;

#[derive(Debug)]
/// A file that doesn't follow LEX-1 or CNF-6, or that system failed to read or handle
pub enum LoadError {
    /// The system failed to open the file at the given path
    Read {
        /// The path of the file
        path: String,
        /// The original system error text
        system_error: String,
    },
    /// The file contains at least one invalid utf-8 char, the span holds its position
    InvalidUtf8(Span),
    /// The file is too large and exceeds [`MAX_SOURCE_SIZE`]
    TooLarge {
        /// The file id
        file: FileId,
        /// The limit
        limit: u64,
    },
}

impl Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Read { path, system_error } => {
                write!(f, "cannot read {path}: {system_error}")
            }
            LoadError::InvalidUtf8(_) => write!(f, "source file is not valid UTF-8"),
            LoadError::TooLarge { file, limit } => write!(
                f,
                "source file {file:?} is too large: the limit is {limit} bytes"
            ),
        }
    }
}

impl Error for LoadError {}

/// Load a file into the source map and get its id
///
/// # Errors
///
/// Returns a [`LoadError`] if the file is invalid, too large, or the system can't handle or read it.
/// An invalid or too large file still goes into the source map for diagnostics
pub fn load_file(source_map: &mut SourceMap, path: &Path, name: &str) -> Result<FileId, LoadError> {
    // Use a proxy to mock the limit in the tests
    load_file_with_limit(source_map, path, name, MAX_SOURCE_SIZE)
}

fn load_file_with_limit(
    source_map: &mut SourceMap,
    path: &Path,
    name: &str,
    limit: u64,
) -> Result<FileId, LoadError> {
    let file = File::open(path).map_err(|e| LoadError::Read {
        path: path.to_string_lossy().to_string(),
        system_error: e.to_string(),
    })?;

    let mut buf = Vec::new();
    // take only the `limit` offset and add 1 to check the size (if the resulting buffer is greater, the file is too large),
    // which saves us to load the entire file into memory.
    let _ = file
        .take(limit + 1)
        .read_to_end(&mut buf)
        .map_err(|e| LoadError::Read {
            path: path.to_string_lossy().to_string(),
            system_error: e.to_string(),
        })?;

    if buf.len() as u64 > limit {
        // A file which is too large goes to the source map with empty content for diagnostics
        return Err(LoadError::TooLarge {
            file: source_map.add(name.to_string(), String::new()),
            limit,
        });
    }

    decode(&mut buf)
        .map(|text| source_map.add(name.to_string(), text))
        .map_err(|(first_invalid_byte, lossy_text)| {
            LoadError::InvalidUtf8(Span::new(
                source_map.add(name.to_string(), lossy_text),
                first_invalid_byte,
                first_invalid_byte
                    + u32::try_from('\u{FFFD}'.len_utf8()).expect("\u{FFFD} size fits in a u32"),
            ))
        })
}

/// Decode the file bytes into an UTF-8 string, removing the BOM if present at the start of the text
///
/// # Errors
///
/// If the bytes are invalid, returns the index of the first invalid byte and the lossy version of the text,
/// with invalid characters replaced by U+FFFD
fn decode(buf: &mut Vec<u8>) -> Result<String, (u32, String)> {
    // Remove BOM if present. If it appears in the middle of the text, the lexer will refuse it later
    if buf.starts_with(&[0xEF, 0xBB, 0xBF]) {
        buf.drain(..3);
    }

    String::from_utf8(buf.clone()).map_err(|e| {
        (
            u32::try_from(e.utf8_error().valid_up_to()).expect(
                "at this point, the file size verification made in load_file_with_limit() ensures an offset inside fits in a u32",
            ),
            String::from_utf8_lossy(&e.into_bytes()).into_owned(),
        )
    })
}

#[cfg(test)]
mod tests {
    use crate::Span;

    use super::{FileId, LoadError, SourceMap, decode, load_file_with_limit};
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    fn path_of(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/{}", name))
    }

    fn str_to_bytes(str: &str) -> Vec<u8> {
        String::from(str).into_bytes()
    }

    fn bytes_to_lossy_string(bytes: Vec<u8>) -> String {
        String::from_utf8_lossy(&bytes).to_string()
    }

    #[test]
    fn ascii() {
        assert_eq!(
            decode(&mut str_to_bytes("let x = 1")).unwrap(),
            String::from("let x = 1")
        )
    }

    #[test]
    fn many_bytes() {
        assert_eq!(
            decode(&mut str_to_bytes("é€😀")).unwrap(),
            String::from("é€😀")
        );
    }

    #[test]
    fn bom_at_start() {
        assert_eq!(
            decode(&mut [0xEF, 0xBB, 0xBF, b'a'].to_vec()).unwrap(),
            String::from("a")
        )
    }

    #[test]
    fn continuation_byte_alone() {
        let mut continuation_byte: Vec<u8> = [0x80].to_vec();
        assert_eq!(
            decode(&mut continuation_byte).unwrap_err(),
            (0, bytes_to_lossy_string(continuation_byte))
        );
    }

    #[test]
    fn invalid_byte_in_the_middle() {
        let mut text: Vec<u8> = [b'a', b'b', 0xFF, b'c'].to_vec();
        assert_eq!(
            decode(&mut text).unwrap_err(),
            (2, bytes_to_lossy_string(text))
        );
    }

    #[test]
    fn truncated_at_the_end() {
        let mut text: Vec<u8> = [b'a', 0xC3].to_vec();
        assert_eq!(
            decode(&mut text).unwrap_err(),
            (1, bytes_to_lossy_string(text))
        );
    }

    #[test]
    fn too_large_encoding() {
        let mut text: Vec<u8> = [0xC0, 0x80].to_vec();
        assert_eq!(
            decode(&mut text).unwrap_err(),
            (0, bytes_to_lossy_string(text))
        );
    }

    #[test]
    fn substitute() {
        let mut text: Vec<u8> = [0xED, 0xA0, 0x80].to_vec();
        assert_eq!(
            decode(&mut text).unwrap_err(),
            (0, bytes_to_lossy_string(text))
        );
    }

    #[test]
    fn beyond() {
        let mut text: Vec<u8> = [0xF4, 0x90, 0x80, 0x80].to_vec();
        assert_eq!(
            decode(&mut text).unwrap_err(),
            (0, bytes_to_lossy_string(text))
        );
    }

    #[test]
    fn bom_then_invalid_byte() {
        let mut text: Vec<u8> = [0xEF, 0xBB, 0xBF, 0xFF].to_vec();
        assert_eq!(
            decode(&mut text).unwrap_err(),
            (0, bytes_to_lossy_string(text))
        );
    }

    #[test]
    fn file_size_at_the_limit() {
        let path = path_of("valid.fm");
        let source_map = &mut SourceMap::new();
        let id = load_file_with_limit(source_map, &path, "valid.fm", 10).unwrap();
        let file = source_map.file(id);
        assert_eq!(id, FileId(0));
        assert_eq!(file.text(), fs::read_to_string(path).unwrap())
    }

    #[test]
    fn file_size_too_large() {
        let path = path_of("valid.fm");
        let source_map = &mut SourceMap::new();
        let err = load_file_with_limit(source_map, &path, "valid.fm", 9);
        let Err(LoadError::TooLarge { file, limit }) = err else {
            panic!("the result should be a LoadError::TooLarge")
        };

        assert_eq!(file, FileId(0));
        assert_eq!(limit, 9);
    }

    #[test]
    fn read_error() {
        let err = LoadError::Read {
            path: String::from("src/a.fm"),
            system_error: String::from("a system error message"),
        };
        assert_eq!(
            format!("{err}"),
            String::from("cannot read src/a.fm: a system error message")
        )
    }

    #[test]
    fn invalid_utf8_error() {
        let err = LoadError::InvalidUtf8(Span::new(FileId(0), 4, 9));
        assert_eq!(
            format!("{err}"),
            String::from("source file is not valid UTF-8")
        )
    }

    #[test]
    fn too_large_error() {
        let err = LoadError::TooLarge {
            file: FileId(0),
            limit: 10,
        };
        assert_eq!(
            format!("{err}"),
            String::from("source file FileId(0) is too large: the limit is 10 bytes")
        )
    }
}
