//! The load module integration tests.

use std::{
    fs,
    path::{Path, PathBuf},
};

use frame_source::{FileId, LoadError, SourceMap, load_file};

fn path_of(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/{}", name))
}

#[test]
fn valid() {
    let path = path_of("valid.fm");
    let name = "valid.fm";
    let mut source_map = SourceMap::new();
    let id = load_file(&mut source_map, &path, name).unwrap();
    let file = source_map.file(id);
    assert_eq!(id, FileId(0));
    assert_eq!(name, file.name());
    assert_eq!(fs::read_to_string(path).unwrap(), file.text())
}

#[test]
fn with_bom() {
    let path = path_of("bom.fm");
    let name = "bom.fm";
    let mut source_map = SourceMap::new();
    let id = load_file(&mut source_map, &path, name).unwrap();
    let file = source_map.file(id);
    assert_eq!(id, FileId(0));
    assert_eq!(name, file.name());
    assert_eq!(
        fs::read_to_string(path).unwrap().drain(3..).as_str(),
        file.text()
    )
}

#[test]
fn invalid() {
    let path = path_of("invalid.fm");
    let name = "invalid.fm";
    let mut source_map = SourceMap::new();

    let Err(LoadError::InvalidUtf8(span)) = load_file(&mut source_map, &path, name) else {
        panic!("the result should be a LoadError::InvalidUtf8")
    };

    let file = source_map.file(FileId(0));
    let line_col = file.line_col(14);

    assert_eq!(span.len(), 3);
    assert_eq!(line_col.line(), 2);
    assert_eq!(line_col.column(), 5);
    assert_eq!(name, file.name());
    assert_eq!(file.text(), "let a = 1\nlet � = 2\n");
}

#[test]
fn missing() {
    let p = path_of("missing.fm");
    let mut source_map = SourceMap::new();
    let Err(LoadError::Read { path, system_error }) = load_file(&mut source_map, &p, "missing.fm")
    else {
        panic!("the result should be a LoadError::Read")
    };

    assert_eq!(p, path);
    assert_eq!(system_error, "No such file or directory (os error 2)");
}

#[test]
fn directory() {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut source_map = SourceMap::new();
    let Err(LoadError::Read { path, system_error }) = load_file(&mut source_map, &p, "missing.fm")
    else {
        panic!("the result should be a LoadError::Read")
    };

    assert_eq!(p, path);
    assert_eq!(system_error, "Is a directory (os error 21)");
}
