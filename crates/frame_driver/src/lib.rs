//! Driver: todo!

use std::{
    error::Error,
    ffi::OsStr,
    io,
    path::{Path, PathBuf},
};

use frame_diagnostics::{Diagnostic, Prefix, Primary, RuleCode};
use frame_source::{LoadError, SourceMap, load_file};

const MIB: u64 = 1_048_576; // 1024 * 1024

#[derive(Debug)]
/// todo!
pub enum Target {
    /// todo!
    Script(PathBuf),
    /// todo!
    Package(Option<PathBuf>),
}

#[derive(Debug)]
/// todo!
pub enum TargetError {
    /// todo!
    NotFound {
        /// todo!
        path: PathBuf,
    },
    /// todo!
    NotAScript {
        /// todo!
        path: PathBuf,
    },
    /// todo!
    Unreadable {
        /// todo!
        path: PathBuf,
        /// todo!
        system_error: io::Error,
    },
}

impl std::fmt::Display for TargetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound { path } => {
                write!(f, "target '{}' does not exist", path.display())
            }
            Self::NotAScript { path } => {
                write!(
                    f,
                    "target '{}' is neither a '.fm' file nor a directory",
                    path.display()
                )
            }
            Self::Unreadable { path, system_error } => {
                write!(f, "cannot read '{}': {system_error}", path.display())
            }
        }
    }
}

impl Error for TargetError {}

#[derive(Debug)]
/// todo!
pub struct Session {
    /// todo!
    source_map: SourceMap,
    /// todo!
    diagnostics: Vec<Diagnostic>,
}

impl Session {
    /// todo!
    pub fn new(source_map: SourceMap, diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            source_map,
            diagnostics,
        }
    }

    /// todo!
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// todo!
    pub fn source_map(&self) -> &SourceMap {
        &self.source_map
    }
}

/// todo!
pub fn resolve_target(target: Option<&Path>) -> Result<Target, TargetError> {
    match target {
        Some(path) => resolve_path(path),
        None => Ok(Target::Package(None)),
    }
}

fn resolve_path(path: &Path) -> Result<Target, TargetError> {
    if !path.exists() {
        return Err(TargetError::NotFound {
            path: path.to_path_buf(),
        });
    }

    if path.is_dir() {
        return Ok(Target::Package(Some(path.to_path_buf())));
    }

    if path.extension() != Some(OsStr::new("fm")) {
        return Err(TargetError::NotAScript {
            path: path.to_path_buf(),
        });
    }

    Ok(Target::Script(path.to_path_buf()))
}

/// todo!
pub fn check_script(target: &Path) -> Result<Session, TargetError> {
    let mut source_map = SourceMap::new();
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let load = load_file(&mut source_map, target, &target.display().to_string());

    if load.is_ok() {
        return Ok(Session::new(source_map, diagnostics));
    }

    if let Err(LoadError::Read { path, system_error }) = load {
        return Err(TargetError::Unreadable { path, system_error });
    }

    diagnostics.push(error_to_diagnostic(load.unwrap_err()));
    diagnostics.sort_by_key(|diagnostic| match diagnostic.primary() {
        Primary::File { file } => (source_map.file(*file).name(), None),
        Primary::Span { span, .. } => (source_map.file(span.file()).name(), Some(span.start())),
    });

    Ok(Session::new(source_map, diagnostics))
}

fn error_to_diagnostic(error: LoadError) -> Diagnostic {
    match error {
        LoadError::Read { .. } => unreachable!("todo!"),
        LoadError::InvalidUtf8 { span } => Diagnostic::new(
            RuleCode::new(Prefix::Lex, 1),
            "source file is not valid UTF-8",
            Primary::span_with_message(span, "invalid UTF-8 sequence"),
        ),
        LoadError::TooLarge { file, limit } => Diagnostic::new(
            RuleCode::new(Prefix::Cnf, 6),
            "source file is too large",
            Primary::file(file),
        )
        .with_reason(format!("the limit is {} MiB ({limit} bytes)", limit / MIB)),
    }
}
