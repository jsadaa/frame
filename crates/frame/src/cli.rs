//! The frame cli parsing module

use std::{
    ffi::{OsStr, OsString},
    fmt,
    path::PathBuf,
};

/// The frame cli subcommands, each holding its own specific args
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    /// The run command
    Run {
        /// An optional target path, pointing to a frame file (`*.fm`) or a package root
        target: Option<PathBuf>,
        /// Optional arguments to pass to the targeted program
        program_args: Vec<OsString>,
    },
    /// The build command
    Build {
        /// An optional target path, pointing to a frame file (`*.fm`) or a package root
        target: Option<PathBuf>,
        /// An optional output path
        output: Option<PathBuf>,
    },
    /// The check command
    Check {
        /// An optional target path, pointing to a frame file (`*.fm`) or a package root
        target: Option<PathBuf>,
    },
    /// The test command
    Test {
        /// An optional target path, pointing to a frame file (`*.fm`) or a package root
        target: Option<PathBuf>,
        /// An optional name, filtering the matching tests
        name: Option<String>,
        /// An optional seed, as a u64
        seed: Option<u64>,
    },
    /// The format command
    Fmt {
        /// If true, name the files it would format and fails without fixing if applicable (dry-run)
        check: bool,
        /// Optional paths, a frame file (`*.fm`) or a package root
        paths: Vec<PathBuf>,
    },
    /// The explain command
    Explain {
        /// A frame code, by its reference name (ex: "MUT-4")
        code: String,
    },
    /// The help option
    Help,
    /// The version option
    Version,
}

/// A usage error from the input args
#[derive(Debug, PartialEq, Eq)]
pub enum UsageError {
    /// Some arguments are not valid UTF-8
    InvalidUtf8(String),
    /// The subcommand is missing
    MissingCommand,
    /// The subcommand is unknown
    UnknownCommand(String),
    /// The seed is invalid
    InvalidSeed(String),
    /// The subcommand expects an argument
    CommandExpectsArgument {
        /// The command invoked
        command: &'static str,
        /// The expected argument name
        expected: &'static str,
    },
    /// The option expect a value
    OptionExpectsValue(&'static str),
    /// The option is repeated
    RepeatedOption(&'static str),
    /// An unexpected argument
    UnexpectedArgument(String),
    /// An unknown option
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

/// Parses a command from an argument list
///
/// # Errors
///
/// Returns a `UsageError` when the arguments or the command are invalid
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
                })?)
            }
            _ => target = Some(parse_target(arg, target)?),
        }
    }

    Ok(Command::Build { target, output })
}

fn parse_check(args: &[OsString]) -> Result<Command, UsageError> {
    let mut target: Option<PathBuf> = None;

    for arg in args {
        target = Some(parse_target(arg, target)?);
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
                })?)
            }
            Some("--seed") => {
                seed = Some(parse_option_value(iter.next(), &seed, "--seed", |v| {
                    parse_text(v).and_then(parse_seed)
                })?)
            }
            _ => target = Some(parse_target(arg, target)?),
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
            Some(unknown) if is_option(arg) => {
                return Err(UsageError::UnknownOption(unknown.to_string()));
            }
            None if is_option(arg) => {
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
        [_, other, ..] => Err(UsageError::UnexpectedArgument(lossy_string(other))),
        [code] => Ok(Command::Explain {
            code: parse_text(code)?,
        }), // we can convert with loss as the guard is already applied above
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

fn parse_target(target: &OsStr, actual: Option<PathBuf>) -> Result<PathBuf, UsageError> {
    match target.to_str() {
        Some(t) if is_option(target) => Err(UsageError::UnknownOption(t.to_string())),
        None if is_option(target) => Err(UsageError::UnknownOption(lossy_string(target))),
        Some(t) if actual.is_some() => Err(UsageError::UnexpectedArgument(t.to_string())),
        _ => Ok(PathBuf::from(target)),
    }
}

fn parse_option_value<T>(
    value: Option<&OsString>,
    current: &Option<T>,
    option: &'static str,
    convert: impl FnOnce(&OsStr) -> Result<T, UsageError>,
) -> Result<T, UsageError> {
    if current.is_some() {
        return Err(UsageError::RepeatedOption(option));
    }

    value
        .ok_or(UsageError::OptionExpectsValue(option))
        .and_then(|v| convert(v))
}

fn parse_seed(seed: String) -> Result<u64, UsageError> {
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
        .ok_or(UsageError::InvalidUtf8(lossy_string(arg)))
}

// Utilities

fn lossy_string(arg: &OsStr) -> String {
    arg.to_string_lossy().into_owned()
}

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
    fn rejects_empty_args() {
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
    fn build_rejects_repeated_option() {
        assert_fails_with(
            parse(&["build", "-o", "a", "-o", "b"]),
            UsageError::RepeatedOption("-o"),
            "option '-o' is repeated",
        );
    }

    #[test]
    fn build_rejects_second_targets() {
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
    fn check_rejects_second_targets() {
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
    fn test_rejects_name_with_no_value_fails() {
        assert_fails_with(
            parse(&["test", "--name"]),
            UsageError::OptionExpectsValue("--name"),
            "option '--name' expects a value",
        );
    }

    #[test]
    fn test_rejects_repeated_names() {
        assert_fails_with(
            parse(&["test", "--name", "a.fm", "--name", "b.fm"]),
            UsageError::RepeatedOption("--name"),
            "option '--name' is repeated",
        );
    }

    #[test]
    fn test_rejects_second_targets() {
        assert_fails_with(
            parse(&["test", "a.fm", "b.fm"]),
            UsageError::UnexpectedArgument("b.fm".to_string()),
            "unexpected argument 'b.fm'",
        );
    }

    #[test]
    fn test_rejects_repeated_seeds() {
        assert_fails_with(
            parse(&["test", "--seed", "77", "--seed", "77"]),
            UsageError::RepeatedOption("--seed"),
            "option '--seed' is repeated",
        );
    }

    #[test]
    fn parses_fmt() {
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
    fn fmt_accepts_paths_and_no_flag() {
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
    fn fmt_rejects_repeated_flags() {
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
    fn explain_rejects_no_code() {
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

        fn args_list_with_invalid_utf8(args: &[&str]) -> Vec<OsString> {
            let non_utf8 = OsString::from_vec(vec![0x66, 0xFF]);
            let mut args: Vec<OsString> = args.iter().map(OsString::from).collect();

            args.push(non_utf8.clone());

            args
        }

        #[test]
        fn rejects_non_utf8_subcommand() {
            let args = args_list_with_invalid_utf8(&[]);
            let non_utf8 = lossy_string(&OsString::from_vec(vec![0x66, 0xFF]));

            assert_fails_with(
                parse_args(&args),
                UsageError::UnknownCommand(non_utf8.clone()),
                format!("unknown command '{}'", non_utf8).as_str(),
            );
        }

        #[test]
        fn test_rejects_non_utf8_name() {
            let args = args_list_with_invalid_utf8(&["test", "--name"]);
            let non_utf8 = lossy_string(&OsString::from_vec(vec![0x66, 0xFF]));

            assert_fails_with(
                parse_args(&args),
                UsageError::InvalidUtf8(non_utf8.clone()),
                format!("argument '{}' is not valid UTF-8", non_utf8,).as_str(),
            );
        }

        #[test]
        fn explain_rejects_non_utf8_code() {
            let args = args_list_with_invalid_utf8(&["explain"]);
            let non_utf8 = lossy_string(&OsString::from_vec(vec![0x66, 0xFF]));

            assert_fails_with(
                parse_args(&args),
                UsageError::InvalidUtf8(non_utf8.clone()),
                format!("argument '{}' is not valid UTF-8", non_utf8,).as_str(),
            );
        }

        #[test]
        fn accepts_non_utf8_paths_as_is() {
            let non_utf8_path = Some(PathBuf::from(OsString::from_vec(vec![0x66, 0xFF])));

            let cases: [(Result<Command, UsageError>, Vec<OsString>); 6] = [
                (
                    Ok(Command::Run {
                        target: non_utf8_path.clone(),
                        program_args: Vec::new(),
                    }),
                    args_list_with_invalid_utf8(&["run"]),
                ),
                (
                    Ok(Command::Build {
                        target: non_utf8_path.clone(),
                        output: None,
                    }),
                    args_list_with_invalid_utf8(&["build"]),
                ),
                (
                    Ok(Command::Build {
                        target: None,
                        output: non_utf8_path.clone(),
                    }),
                    args_list_with_invalid_utf8(&["build", "-o"]),
                ),
                (
                    Ok(Command::Check {
                        target: non_utf8_path.clone(),
                    }),
                    args_list_with_invalid_utf8(&["check"]),
                ),
                (
                    Ok(Command::Test {
                        target: non_utf8_path.clone(),
                        name: None,
                        seed: None,
                    }),
                    args_list_with_invalid_utf8(&["test"]),
                ),
                (
                    Ok(Command::Fmt {
                        check: false,
                        paths: vec![non_utf8_path.clone().unwrap()],
                    }),
                    args_list_with_invalid_utf8(&["fmt"]),
                ),
            ];

            for (cmd, args) in cases {
                assert_eq!(parse_args(&args), cmd, "{args:?}");
            }
        }
    }
}
