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
    /// The file contains invalid utf-8 bytes, the span holds its position
    InvalidUtf8(Span),
    /// The file is too large and exceeds [`MAX_SOURCE_SIZE`]
    TooLarge {
        /// The file id
        file: FileId,
        /// The limit
        limit: u64,
    },
    /// An IO error occured while reading the file content
    Io(io::Error),
}

impl From<io::Error> for LoadError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
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
                "`source file {file:?} is too large: the limit is {limit} bytes`"
            ),
            LoadError::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl Error for LoadError {}

/// Load a file into the source map and get its id
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
    let _handle = file.take(limit + 1).read(&mut buf)?;

    if buf.len() as u64 > limit {
        // A file which is too large goes to the source map with empty content for diagnostics
        return Err(LoadError::TooLarge {
            file: source_map.add(name.to_string(), "".to_string()),
            limit,
        });
    }

    decode(&buf)
        .map(|text| source_map.add(name.to_string(), text))
        .map_err(|(first_invalid_byte, invalid_text)| {
            LoadError::InvalidUtf8(Span {
                // An invalid utf-8 file goes into the source map for diagnostics
                file: source_map.add(name.to_string(), invalid_text),
                start: first_invalid_byte,
                end: u32::try_from('\u{FFFD}'.len_utf8()).expect("\u{FFFD} size fits in a u32"),
            })
        })
}

/// Decode the file bytes into an UTF-8 string or return the index of the first invalid bytes and the lossy version of the text with invalid characters replaced by U+FFFD
fn decode(buf: &[u8]) -> Result<String, (u32, String)> {
    // Remove bytes order mark if present. If some appears in the middle of the text, the lexer will refuse it later
    let bytes = if buf.starts_with(&[0xEF, 0xBB, 0xBF]) {
        buf.to_vec().drain(..3).collect()
    } else {
        buf.to_vec()
    };

    String::from_utf8(bytes).map_err(|e| {
        (
            u32::try_from(e.utf8_error().valid_up_to()).expect(
                "at this point, the file size verification ensures an offset inside fits in a u32",
            ),
            String::from_utf8_lossy(&e.into_bytes()).into_owned(),
        )
    })
}
