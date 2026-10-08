//! The `frame` source crate.
//!
//! Handle the position of chunks and characters in a source file

use std::{collections::HashMap, fs::File};

/// A source file id
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FileId(u32);

/// A source file
#[derive(Debug)]
pub struct SourceFile {
    /// The displayed name
    name: String,
    /// The text content of the source file
    text: String,
}

/// A collection of source files
#[derive(Debug)]
pub struct SourceMap {
    inner: HashMap<FileId, SourceFile>,
}

impl SourceMap {
    /// Add a source file and get its id
    ///
    /// # Panics
    ///
    /// Panics if the length of the inner `HashMap` exceed the size of a u32 integer.
    pub fn add(&mut self, source_file: SourceFile) -> FileId {
        let id = FileId(
            u32::try_from(self.inner.len() + 1)
                .expect("a file id should be in the range of a u32 integer"),
        );

        self.inner.insert(id, source_file);

        id
    }

    /// Get the name of a source file
    pub fn name(&self, id: &FileId) -> Option<String> {
        self.inner
            .iter()
            .find(|(k, _)| id == *k)
            .map(|(_, file)| file.name.clone())

        // Or :
        // ```
        // if !self.inner.contains_key(id) {
        //     return None;
        // }
        // Some(self.inner[id].name.clone())
        // ```
        //
        // ? Which is more idiomatic and efficient ?
    }

    /// Get the text content of a source file
    pub fn text(&self, id: &FileId) -> Option<String> {
        self.inner
            .iter()
            .find(|(k, _)| id == *k)
            .map(|(_, file)| file.text.clone())
    }
}

/// A chunk of text in a `File`, from `start` to `end`
#[allow(dead_code)]
#[derive(Debug)]
pub struct Span {
    /// The source file on the file system
    file: File,
    /// start of the bytes offset in the text,
    start: u32,
    /// end of the bytes offset in the text,
    end: u32,
}

impl Span {
    /// Check if the span offset is 0
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Returns the length of the span
    pub fn len(&self) -> u32 {
        self.end - self.start
    }
}
