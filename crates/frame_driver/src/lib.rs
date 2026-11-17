//! The driver: runs the phases of the compiler on a target, and gathers their diagnostics.
//!
//! [`resolve_target`] tells what a command works on (CLI-2); [`check_script`] loads a script
//! and returns its [`Session`]. The driver writes nothing: the command renders the
//! diagnostics and chooses the exit code.

use std::{
    error::Error,
    ffi::OsStr,
    fs,
    io::{self, ErrorKind},
    path::{Path, PathBuf},
};

use frame_diagnostics::{Diagnostic, Prefix, Primary, RuleCode};
use frame_source::{LoadError, SourceMap, load_file};

const MIB: u64 = 1024 * 1024;

/// What a command works on (CLI-2).
#[derive(Debug)]
pub enum Target {
    /// A `.fm` file: a script (SCR-1).
    Script(PathBuf),
    /// A package (PKG-1): its directory, or `None` for the nearest package that contains
    /// the current directory (S-171).
    Package(Option<PathBuf>),
}

/// A target that a command cannot work on: a usage error (CLI-9).
#[derive(Debug)]
pub enum TargetError {
    /// Nothing exists at the path.
    NotFound {
        /// The target, as given.
        path: PathBuf,
    },
    /// The path is a file without the `.fm` extension.
    NotAScript {
        /// The target, as given.
        path: PathBuf,
    },
    /// The system cannot read the target, or tell what it is.
    Unreadable {
        /// The target, as given.
        path: PathBuf,
        /// The system error.
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

/// What the phases share: the files, and the diagnostics in the order of DIA-4. Only the
/// driver makes one.
#[derive(Debug)]
pub struct Session {
    source_map: SourceMap,
    diagnostics: Vec<Diagnostic>,
}

impl Session {
    /// Returns the diagnostics, in the order of DIA-4.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Returns the files that the diagnostics designate.
    pub fn source_map(&self) -> &SourceMap {
        &self.source_map
    }
}

/// Tells what `target` designates (CLI-2): a `.fm` file is a script, a directory a package,
/// and no target the nearest package.
///
/// # Errors
///
/// Returns a [`TargetError`] if nothing exists at `target`, if it is a file without the `.fm`
/// extension, or if the system cannot tell what it is.
pub fn resolve_target(target: Option<&Path>) -> Result<Target, TargetError> {
    match target {
        Some(path) => resolve_path(path),
        None => Ok(Target::Package(None)),
    }
}

fn resolve_path(path: &Path) -> Result<Target, TargetError> {
    // `Path::exists` answers `false` to any error, a permission denied included: it would say
    // that an unreadable target does not exist.
    match fs::metadata(path) {
        Ok(metadata) => {
            if metadata.is_dir() {
                return Ok(Target::Package(Some(path.to_path_buf())));
            }

            if path.extension() != Some(OsStr::new("fm")) {
                return Err(TargetError::NotAScript {
                    path: path.to_path_buf(),
                });
            }

            Ok(Target::Script(path.to_path_buf()))
        }
        Err(error) => match error.kind() {
            ErrorKind::NotFound => Err(TargetError::NotFound {
                path: path.to_path_buf(),
            }),
            _ => Err(TargetError::Unreadable {
                path: path.to_path_buf(),
                system_error: error,
            }),
        },
    }
}

/// Loads the script at `path`, named as given (DIA-5), and returns its session. A file that
/// is not valid UTF-8 (LEX-1) or too large (CNF-6) gives a diagnostic.
///
/// # Errors
///
/// Returns [`TargetError::Unreadable`] if the system cannot read the file.
pub fn check_script(path: &Path) -> Result<Session, TargetError> {
    let mut source_map = SourceMap::new();
    let mut diagnostics = Vec::new();

    if let Err(error) = load_file(&mut source_map, path, &path.display().to_string()) {
        diagnostics.push(load_error_diagnostic(error)?);
    }

    sort_diagnostics(&mut diagnostics, &source_map);

    Ok(Session {
        source_map,
        diagnostics,
    })
}

/// Sorts by file name, then by start of the primary location (DIA-4). A whole file has no
/// start, and `None` sorts first: its diagnostic leads its file. The sort is stable, so
/// diagnostics of the same key keep their order.
fn sort_diagnostics(diagnostics: &mut [Diagnostic], source_map: &SourceMap) {
    diagnostics.sort_by_key(|diagnostic| match diagnostic.primary() {
        Primary::File { file } => (source_map.file(*file).name(), None),
        Primary::Span { span, .. } => (source_map.file(span.file()).name(), Some(span.start())),
    });
}

/// Turns a load error into its diagnostic. A file that cannot be read has none: it is not in
/// the source map, and the error is the target's.
fn load_error_diagnostic(error: LoadError) -> Result<Diagnostic, TargetError> {
    match error {
        LoadError::Read { path, system_error } => {
            Err(TargetError::Unreadable { path, system_error })
        }
        LoadError::InvalidUtf8 { span } => Ok(Diagnostic::new(
            RuleCode::new(Prefix::Lex, 1),
            "source file is not valid UTF-8",
            Primary::span_with_message(span, "invalid UTF-8 sequence"),
        )),
        LoadError::TooLarge { file, limit } => Ok(Diagnostic::new(
            RuleCode::new(Prefix::Cnf, 6),
            "source file is too large",
            Primary::file(file),
        )
        .with_reason(format!("the limit is {} MiB ({limit} bytes)", limit / MIB))),
    }
}
