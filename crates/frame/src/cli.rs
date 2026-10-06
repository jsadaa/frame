//! Parsing of the `frame` command line (CLI-1 to CLI-9).
//!
//! [`parse_args`] turns the arguments into a [`Command`], or refuses them with a
//! [`UsageError`]. It is pure: it reads no environment and prints nothing.

use std::{
    ffi::{OsStr, OsString},
    fmt,
    path::PathBuf,
};

/// A `frame` command line, parsed but not executed.
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    /// `frame run [target [arguments...]]`: compiles in debug mode, then runs (CLI-3).
    Run {
        /// The script (`.fm`) or package directory; `None` means the enclosing package (CLI-2).
        target: Option<PathBuf>,
        /// The arguments that follow the target, passed to the program as given.
        program_args: Vec<OsString>,
    },
    /// `frame build [target] [-o output]`: produces an executable (CLI-4).
    Build {
        /// The script (`.fm`) or package directory; `None` means the enclosing package (CLI-2).
        target: Option<PathBuf>,
        /// Where `-o` writes the executable; `None` means the current directory.
        output: Option<PathBuf>,
    },
    /// `frame check [target]`: checks the program and its tests (CLI-5).
    Check {
        /// The script (`.fm`) or package directory; `None` means the enclosing package (CLI-2).
        target: Option<PathBuf>,
    },
    /// `frame test [target] [--name text] [--seed seed]`: runs the tests (CLI-6).
    Test {
        /// The script (`.fm`) or package directory; `None` means the enclosing package (CLI-2).
        target: Option<PathBuf>,
        /// With `--name`, only the tests whose name contains this text run.
        name: Option<String>,
        /// With `--seed`, the seed of the run, written as 1 to 16 hexadecimal digits.
        seed: Option<u64>,
    },
    /// `frame fmt [--check] [paths...]`: formats source files (CLI-7).
    Fmt {
        /// With `--check`, nothing is rewritten: the command names the files that would
        /// change, and fails if there are any.
        check: bool,
        /// The files and directories to format; empty means the sources of the enclosing package.
        paths: Vec<PathBuf>,
    },
    /// `frame explain code`: explains a rule (CLI-8).
    Explain {
        /// The code of the rule, such as `MUT-4`.
        code: String,
    },
    /// `frame --help`: describes the subcommands (CLI-1).
    Help,
    /// `frame --version`: gives the version of the toolchain (CLI-1).
    Version,
}

/// A command line that does not follow CLI-1: a usage error (CLI-9).
#[derive(Debug, PartialEq, Eq)]
pub enum UsageError {
    /// An argument that must be text, such as a test name, a seed or a rule code, is not
    /// valid UTF-8.
    InvalidUtf8(String),
    /// No subcommand is given.
    MissingCommand,
    /// The subcommand is not one of CLI-1.
    UnknownCommand(String),
    /// The seed is not 1 to 16 hexadecimal digits (CLI-6).
    InvalidSeed(String),
    /// The subcommand lacks its required argument.
    CommandExpectsArgument {
        /// The subcommand.
        command: &'static str,
        /// What the subcommand expects, article included, such as `a code`.
        expected: &'static str,
    },
    /// The option is given without its value.
    OptionExpectsValue(&'static str),
    /// The option is given more than once.
    RepeatedOption(&'static str),
    /// The argument has no place on the command line, such as a second target.
    UnexpectedArgument(String),
    /// The subcommand accepts no such option.
    UnknownOption(String),
}

impl std::error::Error for UsageError {}

impl fmt::Display for UsageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UsageError::InvalidUtf8(arg) => write!(f, "argument '{arg}' is not valid UTF-8"),
            UsageError::MissingCommand => write!(f, "missing command"),
            UsageError::UnknownCommand(unknown) => write!(f, "unknown command '{unknown}'"),
            UsageError::InvalidSeed(seed) => write!(
                f,
                "invalid seed '{seed}': expected 1 to 16 hexadecimal digits"
            ),
            UsageError::CommandExpectsArgument { command, expected } => {
                write!(f, "command '{command}' expects {expected}")
            }
            UsageError::OptionExpectsValue(option) => {
                write!(f, "option '{option}' expects a value")
            }
            UsageError::RepeatedOption(option) => {
                write!(f, "option '{option}' is repeated")
            }
            UsageError::UnexpectedArgument(arg) => {
                write!(f, "unexpected argument '{arg}'")
            }
            UsageError::UnknownOption(option) => {
                write!(f, "unknown option '{option}'")
            }
        }
    }
}

/// Parses the arguments of `frame`, without the program name.
///
/// # Errors
///
/// Returns a [`UsageError`] when the arguments do not follow CLI-1.
pub fn parse_args(args: &[OsString]) -> Result<Command, UsageError> {
    let Some((subcommand, rest)) = args.split_first() else {
        return Err(UsageError::MissingCommand);
    };

    match subcommand.to_str() {
        Some("run") => parse_run(rest),
        Some("build") => parse_build(rest),
        Some("check") => parse_check(rest),
        Some("test") => parse_test(rest),
        Some("fmt") => parse_fmt(rest),
        Some("explain") => parse_explain(rest),
        Some("--help") => parse_help(rest),
        Some("--version") => parse_version(rest),
        Some(unknown) => Err(UsageError::UnknownCommand(unknown.to_owned())),
        None => Err(UsageError::UnknownCommand(lossy_string(subcommand))),
    }
}

// Command parsers

fn parse_run(args: &[OsString]) -> Result<Command, UsageError> {
    let (target, program_args) = match args {
        [target, ..] if is_option(target) => {
            return Err(UsageError::UnknownOption(lossy_string(target)));
        }
        // Everything after the target belongs to the program, even what looks like an option.
        [target, program_args @ ..] => (Some(PathBuf::from(target)), program_args.to_vec()),
        [] => (None, Vec::new()),
    };

    Ok(Command::Run {
        target,
        program_args,
    })
}

fn parse_build(args: &[OsString]) -> Result<Command, UsageError> {
    let mut target: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;

    let mut iter = args.iter();

    while let Some(arg) = iter.next() {
        match arg.to_str() {
            Some("-o") => {
                output = Some(parse_option_value(iter.next(), &output, "-o", |v| {
                    Ok(PathBuf::from(v))
                })?);
            }
            _ => target = Some(parse_target(arg, &target)?),
        }
    }

    Ok(Command::Build { target, output })
}

fn parse_check(args: &[OsString]) -> Result<Command, UsageError> {
    let mut target: Option<PathBuf> = None;

    for arg in args {
        target = Some(parse_target(arg, &target)?);
    }

    Ok(Command::Check { target })
}

fn parse_test(args: &[OsString]) -> Result<Command, UsageError> {
    let mut target: Option<PathBuf> = None;
    let mut name: Option<String> = None;
    let mut seed: Option<u64> = None;

    let mut iter = args.iter();

    while let Some(arg) = iter.next() {
        match arg.to_str() {
            Some("--name") => {
                name = Some(parse_option_value(iter.next(), &name, "--name", |v| {
                    parse_text(v)
                })?);
            }
            Some("--seed") => {
                seed = Some(parse_option_value(iter.next(), &seed, "--seed", |v| {
                    parse_text(v).and_then(parse_seed)
                })?);
            }
            _ => target = Some(parse_target(arg, &target)?),
        }
    }

    Ok(Command::Test { target, name, seed })
}

fn parse_fmt(args: &[OsString]) -> Result<Command, UsageError> {
    let mut check = false;
    let mut paths: Vec<PathBuf> = Vec::new();

    for arg in args {
        match arg.to_str() {
            Some("--check") if check => return Err(UsageError::RepeatedOption("--check")),
            Some("--check") => check = true,
            _ if is_option(arg) => {
                return Err(UsageError::UnknownOption(lossy_string(arg)));
            }
            _ => paths.push(PathBuf::from(arg)),
        }
    }

    Ok(Command::Fmt { check, paths })
}

fn parse_explain(args: &[OsString]) -> Result<Command, UsageError> {
    match args {
        [code, ..] if is_option(code) => Err(UsageError::UnknownOption(lossy_string(code))),
        [_, extra, ..] => Err(UsageError::UnexpectedArgument(lossy_string(extra))),
        [code] => Ok(Command::Explain {
            code: parse_text(code)?,
        }),
        [] => Err(UsageError::CommandExpectsArgument {
            command: "explain",
            expected: "a code",
        }),
    }
}

fn parse_help(args: &[OsString]) -> Result<Command, UsageError> {
    match args.first() {
        Some(arg) => Err(UsageError::UnexpectedArgument(lossy_string(arg))),
        None => Ok(Command::Help),
    }
}

fn parse_version(args: &[OsString]) -> Result<Command, UsageError> {
    match args.first() {
        Some(arg) => Err(UsageError::UnexpectedArgument(lossy_string(arg))),
        None => Ok(Command::Version),
    }
}

// Utility parsers

fn parse_target(arg: &OsStr, current: &Option<PathBuf>) -> Result<PathBuf, UsageError> {
    if is_option(arg) {
        return Err(UsageError::UnknownOption(lossy_string(arg)));
    }

    if current.is_some() {
        return Err(UsageError::UnexpectedArgument(lossy_string(arg)));
    }

    Ok(PathBuf::from(arg))
}

fn parse_option_value<T>(
    value: Option<&OsString>,
    current: &Option<T>,
    option: &'static str,
    convert: impl FnOnce(&OsStr) -> Result<T, UsageError>,
) -> Result<T, UsageError> {
    // The repetition comes first, so that `-o a -o` reports it rather than the missing value.
    if current.is_some() {
        return Err(UsageError::RepeatedOption(option));
    }

    value
        .ok_or(UsageError::OptionExpectsValue(option))
        .and_then(|v| convert(v))
}

fn parse_seed(seed: String) -> Result<u64, UsageError> {
    // `from_str_radix` accepts a leading `+`: the digits are checked first.
    if !(1usize..=16usize).contains(&seed.chars().count())
        || !seed.chars().all(|c| c.is_ascii_hexdigit())
    {
        return Err(UsageError::InvalidSeed(seed));
    }

    u64::from_str_radix(seed.as_str(), 16).map_err(|_| UsageError::InvalidSeed(seed))
}

fn parse_text(arg: &OsStr) -> Result<String, UsageError> {
    arg.to_str()
        .map(String::from)
        .ok_or_else(|| UsageError::InvalidUtf8(lossy_string(arg)))
}

// Utilities

fn lossy_string(arg: &OsStr) -> String {
    arg.to_string_lossy().into_owned()
}

// The encoded bytes, rather than `to_str`, so that a non-UTF-8 argument that starts with `-`
// is an option too.
fn is_option(arg: &OsStr) -> bool {
    arg.as_encoded_bytes().starts_with(b"-")
}

#[cfg(test)]
mod tests {
    use super::{Command, OsString, PathBuf, UsageError, parse_args};

    fn parse(args: &[&str]) -> Result<Command, UsageError> {
        let args: Vec<OsString> = args.iter().map(OsString::from).collect();
        parse_args(&args)
    }

    fn assert_fails_with(result: Result<Command, UsageError>, error: UsageError, message: &str) {
        assert_eq!(result, Err(error));
        assert_eq!(format!("{}", result.unwrap_err()), message,)
    }

    #[test]
    fn parses_help() {
        assert_eq!(parse(&["--help"]), Ok(Command::Help));
    }

    #[test]
    fn help_rejects_extra_args() {
        assert_fails_with(
            parse(&["--help", "whatever"]),
            UsageError::UnexpectedArgument("whatever".to_string()),
            "unexpected argument 'whatever'",
        );
    }

    #[test]
    fn parses_version() {
        assert_eq!(parse(&["--version"]), Ok(Command::Version));
    }

    #[test]
    fn version_rejects_extra_args() {
        assert_fails_with(
            parse(&["--version", "whatever"]),
            UsageError::UnexpectedArgument("whatever".to_string()),
            "unexpected argument 'whatever'",
        );
    }

    #[test]
    fn rejects_missing_command() {
        assert_fails_with(parse(&[]), UsageError::MissingCommand, "missing command");
    }

    #[test]
    fn rejects_unknown_command() {
        assert_fails_with(
            parse(&["compile"]),
            UsageError::UnknownCommand("compile".to_string()),
            "unknown command 'compile'",
        );
    }

    #[test]
    fn run_accepts_no_args() {
        let program_args: Vec<OsString> = Vec::new();

        assert_eq!(
            parse(&["run"]),
            Ok(Command::Run {
                target: None,
                program_args,
            })
        );
    }

    #[test]
    fn run_accepts_target_and_args() {
        assert_eq!(
            parse(&["run", "hello.fm", "Alice"]),
            Ok(Command::Run {
                target: Some(PathBuf::from("hello.fm")),
                program_args: ["Alice"].map(OsString::from).to_vec(),
            })
        );
    }

    #[test]
    fn run_accepts_args_looking_like_options() {
        assert_eq!(
            parse(&["run", ".", "logs", "7", "--dry-run"]),
            Ok(Command::Run {
                target: Some(PathBuf::from(".")),
                program_args: ["logs", "7", "--dry-run"].map(OsString::from).to_vec(),
            })
        );
    }

    #[test]
    fn run_rejects_option_before_target() {
        assert_fails_with(
            parse(&["run", "--fast", "x.fm"]),
            UsageError::UnknownOption("--fast".to_string()),
            "unknown option '--fast'",
        );
    }

    #[test]
    fn build_accepts_output_and_no_target() {
        assert_eq!(
            parse(&["build", "-o", "out"]),
            Ok(Command::Build {
                target: None,
                output: Some(PathBuf::from("out")),
            })
        );
    }

    #[test]
    fn build_accepts_output_and_target() {
        assert_eq!(
            parse(&["build", "app.fm", "-o", "out"]),
            Ok(Command::Build {
                target: Some(PathBuf::from("app.fm")),
                output: Some(PathBuf::from("out"))
            })
        );
    }

    #[test]
    fn build_accepts_output_before_target() {
        assert_eq!(
            parse(&["build", "-o", "out", "app.fm"]),
            Ok(Command::Build {
                target: Some(PathBuf::from("app.fm")),
                output: Some(PathBuf::from("out")),
            })
        );
    }

    #[test]
    fn build_rejects_output_with_no_value() {
        assert_fails_with(
            parse(&["build", "-o"]),
            UsageError::OptionExpectsValue("-o"),
            "option '-o' expects a value",
        );
    }

    #[test]
    fn build_rejects_unknown_option() {
        assert_fails_with(
            parse(&["build", "--verbose"]),
            UsageError::UnknownOption("--verbose".to_string()),
            "unknown option '--verbose'",
        );
    }

    #[test]
    fn build_rejects_repeated_output() {
        assert_fails_with(
            parse(&["build", "-o", "a", "-o", "b"]),
            UsageError::RepeatedOption("-o"),
            "option '-o' is repeated",
        );
    }

    #[test]
    fn build_rejects_second_target() {
        assert_fails_with(
            parse(&["build", "a.fm", "b.fm"]),
            UsageError::UnexpectedArgument("b.fm".to_string()),
            "unexpected argument 'b.fm'",
        );
    }

    #[test]
    fn check_accepts_target() {
        assert_eq!(
            parse(&["check", "app.fm"]),
            Ok(Command::Check {
                target: Some(PathBuf::from("app.fm"))
            })
        );
    }

    #[test]
    fn check_accepts_no_target() {
        assert_eq!(parse(&["check"]), Ok(Command::Check { target: None }));
    }

    #[test]
    fn check_rejects_second_target() {
        assert_fails_with(
            parse(&["check", "a.fm", "b.fm"]),
            UsageError::UnexpectedArgument("b.fm".to_string()),
            "unexpected argument 'b.fm'",
        );
    }

    #[test]
    fn test_accepts_every_option() {
        assert_eq!(
            parse(&[
                "test",
                "test.fm",
                "--name",
                "parse",
                "--seed",
                "7c1f03a9e4b2d856"
            ]),
            Ok(Command::Test {
                target: Some(PathBuf::from("test.fm")),
                name: Some("parse".to_string()),
                seed: Some(0x7c1f03a9e4b2d856),
            })
        );
    }

    #[test]
    fn test_name_option_takes_next_arg() {
        assert_eq!(
            parse(&["test", "--name", "--seed"]),
            Ok(Command::Test {
                target: None,
                name: Some("--seed".to_string()),
                seed: None,
            })
        );
    }

    #[test]
    fn test_accepts_uppercase_seed() {
        assert_eq!(
            parse(&["test", "--seed", "FF"]),
            Ok(Command::Test {
                target: None,
                name: None,
                seed: Some(255),
            })
        );
    }

    #[test]
    fn test_rejects_invalid_seeds() {
        for seed in ["", "7c1f03a9e4b2d8562", "+ff", "0xff", "g1"] {
            assert_fails_with(
                parse(&["test", "--seed", seed]),
                UsageError::InvalidSeed(seed.to_string()),
                format!("invalid seed '{seed}': expected 1 to 16 hexadecimal digits").as_str(),
            );
        }
    }

    #[test]
    fn test_rejects_unknown_option() {
        assert_fails_with(
            parse(&["test", "--verbose"]),
            UsageError::UnknownOption("--verbose".to_string()),
            "unknown option '--verbose'",
        );
    }

    #[test]
    fn test_rejects_seed_with_no_value() {
        assert_fails_with(
            parse(&["test", "--seed"]),
            UsageError::OptionExpectsValue("--seed"),
            "option '--seed' expects a value",
        );
    }

    #[test]
    fn test_rejects_name_with_no_value() {
        assert_fails_with(
            parse(&["test", "--name"]),
            UsageError::OptionExpectsValue("--name"),
            "option '--name' expects a value",
        );
    }

    #[test]
    fn test_rejects_repeated_name() {
        assert_fails_with(
            parse(&["test", "--name", "a.fm", "--name", "b.fm"]),
            UsageError::RepeatedOption("--name"),
            "option '--name' is repeated",
        );
    }

    #[test]
    fn test_rejects_second_target() {
        assert_fails_with(
            parse(&["test", "a.fm", "b.fm"]),
            UsageError::UnexpectedArgument("b.fm".to_string()),
            "unexpected argument 'b.fm'",
        );
    }

    #[test]
    fn test_rejects_repeated_seed() {
        assert_fails_with(
            parse(&["test", "--seed", "77", "--seed", "77"]),
            UsageError::RepeatedOption("--seed"),
            "option '--seed' is repeated",
        );
    }

    #[test]
    fn fmt_accepts_no_args() {
        let paths: Vec<PathBuf> = Vec::new();

        assert_eq!(
            parse(&["fmt"]),
            Ok(Command::Fmt {
                check: false,
                paths,
            })
        );
    }

    #[test]
    fn fmt_accepts_check_flag() {
        let paths: Vec<PathBuf> = Vec::new();

        assert_eq!(
            parse(&["fmt", "--check"]),
            Ok(Command::Fmt { check: true, paths })
        );
    }

    #[test]
    fn fmt_accepts_paths() {
        assert_eq!(
            parse(&["fmt", "a.fm", "src"]),
            Ok(Command::Fmt {
                check: false,
                paths: ["a.fm", "src"].map(PathBuf::from).to_vec(),
            })
        );
    }

    #[test]
    fn fmt_accepts_path_before_flag() {
        assert_eq!(
            parse(&["fmt", "src", "--check"]),
            Ok(Command::Fmt {
                check: true,
                paths: ["src"].map(PathBuf::from).to_vec()
            })
        );
    }

    #[test]
    fn fmt_rejects_repeated_check() {
        let result = parse(&["fmt", "--check", "--check"]);
        assert_fails_with(
            result,
            UsageError::RepeatedOption("--check"),
            "option '--check' is repeated",
        );
    }

    #[test]
    fn fmt_rejects_unknown_option() {
        assert_fails_with(
            parse(&["fmt", "--verbose"]),
            UsageError::UnknownOption("--verbose".to_string()),
            "unknown option '--verbose'",
        );
    }

    #[test]
    fn explain_accepts_code() {
        assert_eq!(
            parse(&["explain", "MUT-4"]),
            Ok(Command::Explain {
                code: "MUT-4".to_string()
            })
        );
    }

    #[test]
    fn explain_rejects_missing_code() {
        assert_fails_with(
            parse(&["explain"]),
            UsageError::CommandExpectsArgument {
                command: "explain",
                expected: "a code",
            },
            "command 'explain' expects a code",
        );
    }

    #[test]
    fn explain_rejects_second_code() {
        assert_fails_with(
            parse(&["explain", "MUT-4", "MUT-5"]),
            UsageError::UnexpectedArgument("MUT-5".to_string()),
            "unexpected argument 'MUT-5'",
        );
    }

    #[test]
    fn check_rejects_unknown_option() {
        assert_fails_with(
            parse(&["check", "--verbose"]),
            UsageError::UnknownOption("--verbose".to_string()),
            "unknown option '--verbose'",
        );
    }

    #[cfg(unix)]
    mod non_utf8 {
        use super::super::{Command, OsString, PathBuf, UsageError, lossy_string, parse_args};
        use super::assert_fails_with;
        use std::os::unix::ffi::OsStringExt;

        fn non_utf8_arg() -> OsString {
            OsString::from_vec(vec![0x66, 0xFF])
        }

        fn non_utf8_option() -> OsString {
            OsString::from_vec(vec![0x2D, 0xFF])
        }

        fn args_with_non_utf8_arg(args: &[&str]) -> Vec<OsString> {
            let mut args: Vec<OsString> = args.iter().map(OsString::from).collect();

            args.push(non_utf8_arg());

            args
        }

        fn args_with_non_utf8_option(args: &[&str]) -> Vec<OsString> {
            let mut args: Vec<OsString> = args.iter().map(OsString::from).collect();

            args.push(non_utf8_option());

            args
        }

        #[test]
        fn rejects_non_utf8_subcommand() {
            let args = args_with_non_utf8_arg(&[]);
            let non_utf8 = lossy_string(&non_utf8_arg());

            assert_fails_with(
                parse_args(&args),
                UsageError::UnknownCommand(non_utf8.clone()),
                format!("unknown command '{}'", non_utf8).as_str(),
            );
        }

        #[test]
        fn test_rejects_non_utf8_name() {
            let args = args_with_non_utf8_arg(&["test", "--name"]);
            let non_utf8 = lossy_string(&non_utf8_arg());

            assert_fails_with(
                parse_args(&args),
                UsageError::InvalidUtf8(non_utf8.clone()),
                format!("argument '{}' is not valid UTF-8", non_utf8,).as_str(),
            );
        }

        #[test]
        fn explain_rejects_non_utf8_code() {
            let args = args_with_non_utf8_arg(&["explain"]);
            let non_utf8 = lossy_string(&non_utf8_arg());

            assert_fails_with(
                parse_args(&args),
                UsageError::InvalidUtf8(non_utf8.clone()),
                format!("argument '{}' is not valid UTF-8", non_utf8,).as_str(),
            );
        }

        #[test]
        fn run_accepts_non_utf8_program_args() {
            let args = args_with_non_utf8_arg(&["run", "a.fm"]);

            assert_eq!(
                parse_args(&args),
                Ok(Command::Run {
                    target: Some(PathBuf::from("a.fm")),
                    program_args: vec![non_utf8_arg()]
                }),
            )
        }

        #[test]
        fn rejects_non_utf8_second_target() {
            let cases = [
                args_with_non_utf8_arg(&["build", "a.fm"]),
                args_with_non_utf8_arg(&["check", "a.fm"]),
                args_with_non_utf8_arg(&["test", "a.fm"]),
            ];

            let lossy = lossy_string(&non_utf8_arg());

            for args in cases {
                assert_fails_with(
                    parse_args(&args),
                    UsageError::UnexpectedArgument(lossy.clone()),
                    format!("unexpected argument '{}'", lossy).as_str(),
                );
            }
        }

        #[test]
        fn rejects_non_utf8_option() {
            let cases = [
                args_with_non_utf8_option(&["build"]),
                args_with_non_utf8_option(&["check"]),
                args_with_non_utf8_option(&["test"]),
                args_with_non_utf8_option(&["fmt"]),
            ];

            let lossy = lossy_string(&non_utf8_option());

            for args in cases {
                println!("{args:?}");
                assert_fails_with(
                    parse_args(&args),
                    UsageError::UnknownOption(lossy.clone()),
                    format!("unknown option '{}'", lossy).as_str(),
                );
            }
        }

        #[test]
        fn accepts_non_utf8_paths_as_is() {
            let non_utf8_path = Some(PathBuf::from(non_utf8_arg()));

            let cases = [
                (
                    Command::Run {
                        target: non_utf8_path.clone(),
                        program_args: Vec::new(),
                    },
                    args_with_non_utf8_arg(&["run"]),
                ),
                (
                    Command::Build {
                        target: non_utf8_path.clone(),
                        output: None,
                    },
                    args_with_non_utf8_arg(&["build"]),
                ),
                (
                    Command::Build {
                        target: None,
                        output: non_utf8_path.clone(),
                    },
                    args_with_non_utf8_arg(&["build", "-o"]),
                ),
                (
                    Command::Check {
                        target: non_utf8_path.clone(),
                    },
                    args_with_non_utf8_arg(&["check"]),
                ),
                (
                    Command::Test {
                        target: non_utf8_path.clone(),
                        name: None,
                        seed: None,
                    },
                    args_with_non_utf8_arg(&["test"]),
                ),
                (
                    Command::Fmt {
                        check: false,
                        paths: vec![PathBuf::from(non_utf8_arg())],
                    },
                    args_with_non_utf8_arg(&["fmt"]),
                ),
            ];

            for (expected, args) in cases {
                assert_eq!(parse_args(&args), Ok(expected), "{args:?}");
            }
        }
    }
}
