//! The driver: runs the phases of the compiler on a target, and gathers their diagnostics.
//!
//! [`resolve_target`] tells what a command works on (CLI-2); [`check_script`] loads a script
//! and returns its [`Session`]. The driver writes nothing: the command renders the
//! diagnostics and chooses the exit code.

use std::{
    error::Error,
    ffi::OsStr,
    fmt, fs,
    io::{self, ErrorKind},
    path::{Path, PathBuf},
};

use frame_diagnostics::{Diagnostic, Prefix, Primary, RuleCode};
use frame_source::{LoadError, SourceMap, load_file};

const MIB: u64 = 1024 * 1024;

/// What a command works on (CLI-2).
#[derive(Debug, PartialEq, Eq)]
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

impl fmt::Display for TargetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { path } => write!(f, "target '{}' does not exist", path.display()),
            Self::NotAScript { path } => write!(
                f,
                "target '{}' is neither a '.fm' file nor a directory",
                path.display()
            ),
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
    let path = path.to_path_buf();

    match fs::metadata(&path) {
        Ok(metadata) if metadata.is_dir() => Ok(Target::Package(Some(path))),
        Ok(_) if path.extension() == Some(OsStr::new("fm")) => Ok(Target::Script(path)),
        Ok(_) => Err(TargetError::NotAScript { path }),
        Err(error) if error.kind() == ErrorKind::NotFound => Err(TargetError::NotFound { path }),
        Err(system_error) => Err(TargetError::Unreadable { path, system_error }),
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

#[cfg(test)]
mod tests {
    use std::{
        io::{self, ErrorKind},
        path::{Path, PathBuf},
    };

    use frame_diagnostics::{Diagnostic, Prefix, Primary, RuleCode};
    use frame_source::{FileId, LoadError, MAX_SOURCE_SIZE, SourceMap, Span};

    use super::{Target, TargetError, load_error_diagnostic, resolve_target, sort_diagnostics};

    const ENCODING: RuleCode = RuleCode::new(Prefix::Lex, 1);
    const TRANSLATION_LIMITS: RuleCode = RuleCode::new(Prefix::Cnf, 6);

    fn fixtures() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    fn whole_file(file: FileId, title: &str) -> Diagnostic {
        Diagnostic::new(TRANSLATION_LIMITS, title, Primary::file(file))
    }

    fn at_span(file: FileId, start: u32, end: u32, title: &str) -> Diagnostic {
        Diagnostic::new(ENCODING, title, Primary::span(Span::new(file, start, end)))
    }

    #[test]
    fn sorts_by_file_name_then_by_start_with_the_whole_file_first() {
        let mut source_map = SourceMap::new();
        // Added before `a.fm`: the order of the ids is not the order of the names.
        let b = source_map.add("b.fm".to_string(), "let b = 1\n".to_string());
        let a = source_map.add("a.fm".to_string(), "let a = 1\n".to_string());

        let mut diagnostics = [
            at_span(b, 0, 1, "b.fm at 0"),
            at_span(a, 8, 9, "a.fm at 8"),
            whole_file(b, "b.fm whole"),
            at_span(b, 5, 6, "b.fm at 5"),
            at_span(a, 2, 3, "a.fm at 2"),
            whole_file(a, "a.fm whole"),
        ];
        sort_diagnostics(&mut diagnostics, &source_map);

        assert_eq!(
            diagnostics,
            [
                whole_file(a, "a.fm whole"),
                at_span(a, 2, 3, "a.fm at 2"),
                at_span(a, 8, 9, "a.fm at 8"),
                whole_file(b, "b.fm whole"),
                at_span(b, 0, 1, "b.fm at 0"),
                at_span(b, 5, 6, "b.fm at 5"),
            ]
        );
    }

    #[test]
    fn diagnostics_of_the_same_key_keep_their_order() {
        let mut source_map = SourceMap::new();
        let file = source_map.add("a.fm".to_string(), "let a = 1\n".to_string());

        // Same start, the first one ending later: a key that took the end too would swap them.
        let mut diagnostics = [
            at_span(file, 4, 9, "at 4, first"),
            whole_file(file, "whole, first"),
            at_span(file, 4, 5, "at 4, second"),
            whole_file(file, "whole, second"),
        ];
        sort_diagnostics(&mut diagnostics, &source_map);

        assert_eq!(
            diagnostics,
            [
                whole_file(file, "whole, first"),
                whole_file(file, "whole, second"),
                at_span(file, 4, 9, "at 4, first"),
                at_span(file, 4, 5, "at 4, second"),
            ]
        );
    }

    #[test]
    fn invalid_utf8_is_lex_1_on_the_first_replacement_character() {
        let mut source_map = SourceMap::new();
        let file = source_map.add(
            "invalid.fm".to_string(),
            "let a = 1\nlet \u{FFFD} = 2\n".to_string(),
        );
        let span = Span::new(file, 14, 17);

        let diagnostic = load_error_diagnostic(LoadError::InvalidUtf8 { span })
            .expect("an invalid file should give a diagnostic");

        assert_eq!(
            diagnostic,
            Diagnostic::new(
                ENCODING,
                "source file is not valid UTF-8",
                Primary::span_with_message(span, "invalid UTF-8 sequence"),
            )
        );
    }

    #[test]
    fn too_large_is_cnf_6_on_the_whole_file_with_the_limit() {
        let mut source_map = SourceMap::new();
        let file = source_map.add("big.fm".to_string(), String::new());

        let diagnostic = load_error_diagnostic(LoadError::TooLarge {
            file,
            limit: MAX_SOURCE_SIZE,
        })
        .expect("a file too large should give a diagnostic");

        assert_eq!(
            diagnostic,
            Diagnostic::new(
                TRANSLATION_LIMITS,
                "source file is too large",
                Primary::file(file),
            )
            .with_reason("the limit is 16 MiB (16777216 bytes)")
        );
    }

    #[test]
    fn fm_file_is_a_script() {
        let path = fixtures().join("valid.fm");
        let target = resolve_target(Some(&path)).expect("an existing .fm file should resolve");

        assert_eq!(target, Target::Script(path));
    }

    #[test]
    fn file_without_the_fm_extension_is_not_a_script() {
        let path = fixtures().join("notes.txt");
        let target = resolve_target(Some(&path));

        let Err(TargetError::NotAScript { path: error_path }) = target else {
            panic!("expected NotAScript, got {target:?}");
        };
        assert_eq!(error_path, path);
    }

    #[test]
    fn missing_path_is_not_found() {
        let path = fixtures().join("missing.fm");
        let target = resolve_target(Some(&path));

        let Err(TargetError::NotFound { path: error_path }) = target else {
            panic!("expected NotFound, got {target:?}");
        };
        assert_eq!(error_path, path);
    }

    #[test]
    fn directory_is_a_package() {
        let path = fixtures();
        let target = resolve_target(Some(&path)).expect("an existing directory should resolve");

        assert_eq!(target, Target::Package(Some(path)));
    }

    #[test]
    fn no_target_is_the_nearest_package() {
        let target = resolve_target(None).expect("no target should resolve");

        assert_eq!(target, Target::Package(None));
    }

    #[test]
    fn target_errors_name_the_target() {
        let cases = [
            (
                TargetError::NotFound {
                    path: PathBuf::from("missing.fm"),
                },
                "target 'missing.fm' does not exist",
            ),
            (
                TargetError::NotAScript {
                    path: PathBuf::from("notes.txt"),
                },
                "target 'notes.txt' is neither a '.fm' file nor a directory",
            ),
            (
                TargetError::Unreadable {
                    path: PathBuf::from("locked.fm"),
                    system_error: io::Error::from(ErrorKind::PermissionDenied),
                },
                "cannot read 'locked.fm': permission denied",
            ),
        ];

        for (error, expected) in cases {
            assert_eq!(error.to_string(), expected, "message of {error:?}");
        }
    }
}
