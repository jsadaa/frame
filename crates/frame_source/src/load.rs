//! Validate and load a source file into the source map

use std::{
    error::Error,
    fmt::Display,
    fs::File,
    io::{self, Read},
    path::Path,
};

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
