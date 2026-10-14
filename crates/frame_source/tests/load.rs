//! Loading files through the public interface of `frame_source`.

use std::{
    io,
    path::{Path, PathBuf},
};

use frame_source::{LoadError, SourceMap, load_file};

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/{name}"))
}

#[test]
fn loads_a_valid_file() {
    let path = fixture_path("valid.fm");
    let name = "valid.fm";
    let mut source_map = SourceMap::new();
    let id = load_file(&mut source_map, &path, name).unwrap();
    let file = source_map.file(id);
    assert_eq!(name, file.name());
    assert_eq!(file.text(), "let a = 1\n")
}

#[test]
fn removes_the_bom() {
    let path = fixture_path("bom.fm");
    let name = "bom.fm";
    let mut source_map = SourceMap::new();
    let id = load_file(&mut source_map, &path, name).unwrap();
    let file = source_map.file(id);
    assert_eq!(name, file.name());
    assert_eq!(file.text(), "let a = 1\n")
}

#[test]
fn locates_the_first_invalid_byte() {
    let path = fixture_path("invalid.fm");
    let name = "invalid.fm";
    let mut source_map = SourceMap::new();

    let result = load_file(&mut source_map, &path, name);
    let Err(LoadError::InvalidUtf8 { span }) = result else {
        panic!("expected an InvalidUtf8 error, got {result:?}")
    };

    let file = source_map.file(span.file());
    let line_col = file.line_col(span.start());

    assert_eq!(span.len(), 3);
    assert_eq!(line_col.line(), 2);
    assert_eq!(line_col.column(), 5);
    assert_eq!(name, file.name());
    assert_eq!(file.text(), "let a = 1\nlet � = 2\n");
}

#[test]
fn missing_file_is_a_read_error() {
    let path = fixture_path("missing.fm");
    let mut source_map = SourceMap::new();
    let result = load_file(&mut source_map, &path, "missing.fm");
    let Err(LoadError::Read {
        path: error_path,
        system_error,
    }) = result
    else {
        panic!("expected a Read error, got {result:?}")
    };

    assert_eq!(path, error_path);
    assert_eq!(system_error.kind(), io::ErrorKind::NotFound);
}

// Under Unix, opening a directory succeeds, and reading it fails with this kind.
#[cfg(unix)]
#[test]
fn directory_is_a_read_error() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut source_map = SourceMap::new();
    let result = load_file(&mut source_map, &path, "fixtures");
    let Err(LoadError::Read {
        path: error_path,
        system_error,
    }) = result
    else {
        panic!("expected a Read error, got {result:?}")
    };

    assert_eq!(path, error_path);
    assert_eq!(system_error.kind(), io::ErrorKind::IsADirectory);
}
