use std::{
    ffi::{OsStr, OsString},
    fmt,
    path::PathBuf,
};

use crate::cli::UsageError::UnexpectedArgument;

/// The frame cli subcommands, each holding its own specific args
#[derive(Debug, PartialEq)]
pub enum Command {
    /// The run command
    Run {
        /// An optional target path, aiming a frame file (`*.fm`) or a package root
        target: Option<PathBuf>,
        /// Optional arguments to pass to the targeted program
        program_args: Vec<OsString>,
    },
    // The build command
    Build {
        /// An optional target path, aiming a frame file (`*.fm`) or a package root
        target: Option<PathBuf>,
        /// An optional output path
        output: Option<PathBuf>,
    },
    /// The check command
    Check {
        /// An optional target path, aiming a frame file (`*.fm`) or a package root
        target: Option<PathBuf>,
    },
    /// The test command
    Test {
        /// An optional target path, aiming a frame test file (`*.fm`)
        target: Option<PathBuf>,
        /// An optional name, filtering the matching tests
        filter: Option<String>,
        /// An optional seed, as an hexadecimal number
        seed: Option<u64>,
    },
    /// The format command
    Fmt {
        /// Dry run flag
        check: bool,
        /// Optional paths, a frame file (`*.fm`) or a package root
        paths: Vec<PathBuf>,
    },
    /// The explain command
    Explain {
        /// A frame code, by its reference name (ex: "MUT-4")
        code: String,
    },
    /// The help command
    Help,
    /// The version command
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
        command: &'static str,
        arg: &'static str,
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
            UsageError::CommandExpectsArgument { command, arg } => {
                write!(f, "command '{command}' expects a {arg}")
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

/// Parse a command from an argument list
///
/// # Errors
///
/// Return a `UsageError` when the arguments or the command are invalid
pub fn parse_args(args: &[OsString]) -> Result<Command, UsageError> {
    let first = args.split_first();

    if let Some((subcommand, rest)) = first {
        return match subcommand.to_str() {
            Some("run") => parse_run(rest),
            Some("build") => parse_build(rest),
            Some("check") => parse_check(rest),
            Some("test") => parse_test(rest),
            Some("fmt") => parse_fmt(rest),
            Some("explain") => parse_explain(rest),
            Some("--help") => parse_help(rest),
            Some("--version") => parse_version(rest),
            Some(unknown) => Err(UsageError::UnknownCommand(unknown.to_owned())),
            None => Err(UsageError::InvalidUtf8(
                subcommand.to_string_lossy().to_string(),
            )),
        };
    }

    Err(UsageError::MissingCommand)
}

// Parsers

fn parse_run(args: &[OsString]) -> Result<Command, UsageError> {
    let (target, program_args) = match args {
        [target, ..] if is_option(target) => {
            return Err(UsageError::UnknownOption(
                target.to_string_lossy().to_string(),
            ));
        }
        [target] => (Some(PathBuf::from(target)), Vec::new()),
        [target, program_args @ ..] => {
            for arg in program_args {
                if !is_valid_utf8(arg) {
                    return Err(UsageError::InvalidUtf8(arg.to_string_lossy().to_string()));
                }
            }

            (Some(PathBuf::from(target)), program_args.to_vec())
        }
        _ => (None, Vec::new()),
    };

    Ok(Command::Run {
        target,
        program_args,
    })
}

fn parse_build(args: &[OsString]) -> Result<Command, UsageError> {
    let mut target: Option<PathBuf> = None;
    let mut output: Option<&OsString> = None;

    let mut iter = args.iter();

    while let Some(arg) = iter.next() {
        match arg.to_str() {
            Some("-o") if output.is_some() => {
                return Err(UsageError::RepeatedOption("-o"));
            }
            Some("-o") => {
                output = iter.next();
                if output.is_none() {
                    return Err(UsageError::OptionExpectsValue("-o"));
                }
            }
            Some(option) if option.starts_with('-') => {
                return Err(UsageError::UnknownOption(option.to_string()));
            }
            Some(option) if target.is_some() => {
                return Err(UsageError::UnexpectedArgument(option.to_string()));
            }
            _ => {
                target = Some(PathBuf::from(arg));
            }
        }
    }

    Ok(Command::Build {
        target,
        output: output.map(PathBuf::from),
    })
}

fn parse_check(args: &[OsString]) -> Result<Command, UsageError> {
    let mut target: Option<PathBuf> = None;
    let iter = args.iter();

    for arg in iter {
        match arg.to_str() {
            Some(option) if is_option(arg) => {
                return Err(UsageError::UnknownOption(option.to_string()));
            }
            Some(path) if target.is_none() => {
                target = Some(PathBuf::from(path));
            }
            _ => return Err(UnexpectedArgument(arg.to_string_lossy().to_string())),
        }
    }

    Ok(Command::Check { target })
}

fn parse_test(args: &[OsString]) -> Result<Command, UsageError> {
    let mut target: Option<PathBuf> = None;
    let mut filter: Option<&OsString> = None;
    let mut seed: Option<&OsString> = None;

    let mut iter = args.iter();

    while let Some(arg) = iter.next() {
        match arg.to_str() {
            Some("--name") if filter.is_some() => {
                return Err(UsageError::RepeatedOption("--name"));
            }
            Some("--name") => {
                filter = match iter.next() {
                    Some(f) if f.to_str().is_none() => {
                        return Err(UsageError::InvalidUtf8(f.to_string_lossy().to_string()));
                    }
                    Some(f) => Some(f),
                    None => {
                        return Err(UsageError::OptionExpectsValue("--name"));
                    }
                };
            }
            Some("--seed") if seed.is_some() => {
                return Err(UsageError::RepeatedOption("--seed"));
            }
            Some("--seed") => {
                seed = match iter.next() {
                    Some(s) if s.to_str().is_none() => {
                        return Err(UsageError::InvalidUtf8(s.to_string_lossy().to_string()));
                    }
                    Some(s) => Some(s),
                    None => {
                        return Err(UsageError::OptionExpectsValue("--seed"));
                    }
                };
            }
            Some(option) if option.starts_with('-') => {
                return Err(UsageError::UnknownOption(option.to_string()));
            }
            Some(arg) if target.is_some() => {
                return Err(UsageError::UnexpectedArgument(arg.to_string()));
            }
            _ => {
                target = Some(PathBuf::from(arg)); // we keep paths as is, even invalids
            }
        }
    }

    let seed: Option<u64> = if let Some(s) = seed {
        let parsed = parse_seed(s);

        if let Err(p) = parsed {
            return Err(p);
        }

        Some(parsed.unwrap())
    } else {
        None
    };

    Ok(Command::Test {
        target,
        filter: filter.and_then(|f| f.to_str().map(String::from)),
        seed,
    })
}

fn parse_fmt(args: &[OsString]) -> Result<Command, UsageError> {
    let mut check = false;
    let mut paths: Vec<PathBuf> = Vec::new();

    let iter = args.iter();

    for arg in iter {
        match arg.to_str() {
            Some("--check") if check => {
                return Err(UsageError::RepeatedOption("--check"));
            }
            Some("--check") => check = true,
            Some(unknown) if is_option(arg) => {
                return Err(UsageError::UnknownOption(unknown.to_string()));
            }
            _ => paths.push(PathBuf::from(arg)),
        }
    }

    Ok(Command::Fmt { check, paths })
}

fn parse_explain(args: &[OsString]) -> Result<Command, UsageError> {
    let code = match args {
        [code, ..] if is_option(code) => {
            return Err(UsageError::UnknownOption(
                code.to_string_lossy().to_string(),
            ));
        }
        [code, ..] if !is_valid_utf8(code) => {
            return Err(UsageError::InvalidUtf8(code.to_string_lossy().to_string()));
        }
        [_, other, ..] => {
            return Err(UsageError::UnexpectedArgument(
                other.to_string_lossy().to_string(),
            ));
        }
        [code] => Some(code.to_string_lossy().to_string()), // we can convert with loss as the guard is already applied above
        _ => None,
    };

    match code {
        Some(code) => Ok(Command::Explain { code }),
        None => Err(UsageError::CommandExpectsArgument {
            command: "explain",
            arg: "code",
        }),
    }
}

fn parse_help(args: &[OsString]) -> Result<Command, UsageError> {
    match args.first() {
        Some(arg) => Err(UsageError::UnexpectedArgument(
            arg.to_string_lossy().to_string(),
        )),
        None => Ok(Command::Help),
    }
}

fn parse_version(args: &[OsString]) -> Result<Command, UsageError> {
    match args.first() {
        Some(arg) => Err(UsageError::UnexpectedArgument(
            arg.to_string_lossy().to_string(),
        )),
        None => Ok(Command::Version),
    }
}

fn parse_seed(arg: &OsStr) -> Result<u64, UsageError> {
    match arg.to_str() {
        Some(arg) => {
            if !(1usize..=16usize).contains(&arg.chars().count())
                || !arg.chars().all(|c| c.is_ascii_hexdigit())
            {
                return Err(UsageError::InvalidSeed(String::from(arg)));
            }

            u64::from_str_radix(arg, 16).map_err(|_| UsageError::InvalidSeed(String::from(arg)))
        }
        None => Err(UsageError::InvalidUtf8(arg.to_string_lossy().to_string())),
    }
}

// Utilities

fn is_option(arg: &OsStr) -> bool {
    arg.as_encoded_bytes().starts_with(b"-")
}

fn is_valid_utf8(arg: &OsStr) -> bool {
    arg.to_str().is_some()
}

#[cfg(test)]
mod tests {
    use super::{Command, OsString, PathBuf, UsageError, parse_args};
    use std::os::unix::ffi::OsStringExt;

    fn parse(args: &[&str]) -> Result<Command, UsageError> {
        let args: Vec<OsString> = args.iter().map(OsString::from).collect();
        parse_args(&args)
    }

    fn assert_fails(result: Result<Command, UsageError>, error: UsageError, message: &str) {
        assert_eq!(result, Err(error));
        assert_eq!(format!("{}", result.unwrap_err()), message,)
    }

    fn invalid_utf8_os_string() -> OsString {
        OsString::from_vec(vec![0x66, 0xFF])
    }

    #[test]
    fn test_help() {
        assert_eq!(parse(&["--help"]), Ok(Command::Help));
    }

    #[test]
    fn test_help_with_extra_arg_fails() {
        assert_fails(
            parse(&["--help", "whatever"]),
            UsageError::UnexpectedArgument("whatever".to_string()),
            "unexpected argument 'whatever'",
        );
    }

    #[test]
    fn test_version() {
        assert_eq!(parse(&["--version"]), Ok(Command::Version));
    }

    #[test]
    fn test_version_with_extra_arg_fails() {
        assert_fails(
            parse(&["--version", "whatever"]),
            UsageError::UnexpectedArgument("whatever".to_string()),
            "unexpected argument 'whatever'",
        );
    }

    #[test]
    fn test_parse_without_argument_fails() {
        assert_fails(parse(&[]), UsageError::MissingCommand, "missing command");
    }

    #[test]
    fn test_unknown_command_fails() {
        assert_fails(
            parse(&["compile"]),
            UsageError::UnknownCommand("compile".to_string()),
            "unknown command 'compile'",
        );
    }

    #[test]
    fn test_run_without_args() {
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
    fn test_run_with_args() {
        assert_eq!(
            parse(&["run", "hello.fm", "Alice"]),
            Ok(Command::Run {
                target: Some(PathBuf::from("hello.fm")),
                program_args: ["Alice"].map(OsString::from).to_vec(),
            })
        );
    }

    #[test]
    fn test_run_with_args_looking_like_options() {
        assert_eq!(
            parse(&["run", ".", "logs", "7", "--dry-run"]),
            Ok(Command::Run {
                target: Some(PathBuf::from(".")),
                program_args: ["logs", "7", "--dry-run"].map(OsString::from).to_vec(),
            })
        );
    }

    #[test]
    fn test_run_with_option_before_target_fails() {
        assert_fails(
            parse(&["run", "--fast", "x.fm"]),
            UsageError::UnknownOption("--fast".to_string()),
            "unknown option '--fast'",
        );
    }

    #[test]
    fn test_build_with_ouput_and_no_target() {
        assert_eq!(
            parse(&["build", "-o", "out"]),
            Ok(Command::Build {
                target: None,
                output: Some(PathBuf::from("out")),
            })
        );
    }

    #[test]
    fn test_build_with_ouput_and_target() {
        assert_eq!(
            parse(&["build", "app.fm", "-o", "out"]),
            Ok(Command::Build {
                target: Some(PathBuf::from("app.fm")),
                output: Some(PathBuf::from("out"))
            })
        );
    }

    #[test]
    fn test_build_with_ouput_before_target() {
        assert_eq!(
            parse(&["build", "-o", "out", "app.fm"]),
            Ok(Command::Build {
                target: Some(PathBuf::from("app.fm")),
                output: Some(PathBuf::from("out")),
            })
        );
    }

    #[test]
    fn test_build_with_ouput_and_no_value_fails() {
        assert_fails(
            parse(&["build", "-o"]),
            UsageError::OptionExpectsValue("-o"),
            "option '-o' expects a value",
        );
    }

    #[test]
    fn test_build_with_unknown_option_fails() {
        assert_fails(
            parse(&["build", "--verbose"]),
            UsageError::UnknownOption("--verbose".to_string()),
            "unknown option '--verbose'",
        );
    }

    #[test]
    fn test_build_with_repeated_option_fails() {
        assert_fails(
            parse(&["build", "-o", "a", "-o", "b"]),
            UsageError::RepeatedOption("-o"),
            "option '-o' is repeated",
        );
    }

    #[test]
    fn test_build_with_two_targets_fails() {
        assert_fails(
            parse(&["build", "a.fm", "b.fm"]),
            UsageError::UnexpectedArgument("b.fm".to_string()),
            "unexpected argument 'b.fm'",
        );
    }

    #[test]
    fn test_check_with_target() {
        assert_eq!(
            parse(&["check", "app.fm"]),
            Ok(Command::Check {
                target: Some(PathBuf::from("app.fm"))
            })
        );
    }

    #[test]
    fn test_check_without_target() {
        assert_eq!(parse(&["check"]), Ok(Command::Check { target: None }));
    }

    #[test]
    fn test_check_with_duplicated_target_fails() {
        assert_fails(
            parse(&["check", "a.fm", "b.fm"]),
            UsageError::UnexpectedArgument("b.fm".to_string()),
            "unexpected argument 'b.fm'",
        );
    }

    #[test]
    fn test_test() {
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
                filter: Some("parse".to_string()),
                seed: Some(0x7c1f03a9e4b2d856),
            })
        );
    }

    #[test]
    fn test_test_name_option_takes_next_arg() {
        assert_eq!(
            parse(&["test", "--name", "--seed"]),
            Ok(Command::Test {
                target: None,
                filter: Some("--seed".to_string()),
                seed: None,
            })
        );
    }

    #[test]
    fn test_test_uppercase_seed() {
        assert_eq!(
            parse(&["test", "--seed", "FF"]),
            Ok(Command::Test {
                target: None,
                filter: None,
                seed: Some(255),
            })
        );
    }

    #[test]
    fn test_test_with_invalid_seeds_fails() {
        for seed in ["", "7c1f03a9e4b2d8562", "+ff", "0xff", "g1"] {
            assert_fails(
                parse(&["test", "--seed", seed]),
                UsageError::InvalidSeed(seed.to_string()),
                format!("invalid seed '{seed}': expected 1 to 16 hexadecimal digits").as_str(),
            );
        }
    }

    #[test]
    fn test_test_with_unknown_option_fails() {
        assert_fails(
            parse(&["test", "--verbose"]),
            UsageError::UnknownOption("--verbose".to_string()),
            "unknown option '--verbose'",
        );
    }

    #[test]
    fn test_test_with_seed_and_no_value_fails() {
        assert_fails(
            parse(&["test", "--seed"]),
            UsageError::OptionExpectsValue("--seed"),
            "option '--seed' expects a value",
        );
    }

    #[test]
    fn test_test_with_name_and_no_value_fails() {
        assert_fails(
            parse(&["test", "--name"]),
            UsageError::OptionExpectsValue("--name"),
            "option '--name' expects a value",
        );
    }

    #[test]
    fn test_test_with_two_names_fails() {
        assert_fails(
            parse(&["test", "--name", "a.fm", "--name", "b.fm"]),
            UsageError::RepeatedOption("--name"),
            "option '--name' is repeated",
        );
    }

    #[test]
    fn test_test_with_two_targets_fails() {
        assert_fails(
            parse(&["test", "a.fm", "b.fm"]),
            UsageError::UnexpectedArgument("b.fm".to_string()),
            "unexpected argument 'b.fm'",
        );
    }

    #[test]
    fn test_test_with_two_seed_fails() {
        assert_fails(
            parse(&["test", "--seed", "77", "--seed", "77"]),
            UsageError::RepeatedOption("--seed"),
            "option '--seed' is repeated",
        );
    }

    #[test]
    fn test_fmt() {
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
    fn test_fmt_with_check_flag() {
        let paths: Vec<PathBuf> = Vec::new();

        assert_eq!(
            parse(&["fmt", "--check"]),
            Ok(Command::Fmt { check: true, paths })
        );
    }

    #[test]
    fn test_fmt_paths_only() {
        assert_eq!(
            parse(&["fmt", "a.fm", "src"]),
            Ok(Command::Fmt {
                check: false,
                paths: ["a.fm", "src"].map(PathBuf::from).to_vec(),
            })
        );
    }

    #[test]
    fn test_fmt_with_path_before_flag() {
        assert_eq!(
            parse(&["fmt", "src", "--check"]),
            Ok(Command::Fmt {
                check: true,
                paths: ["src"].map(PathBuf::from).to_vec()
            })
        );
    }

    #[test]
    fn test_fmt_with_two_flags_fails() {
        let result = parse(&["fmt", "--check", "--check"]);
        assert_fails(
            result,
            UsageError::RepeatedOption("--check"),
            "option '--check' is repeated",
        );
    }

    #[test]
    fn test_fmt_with_unknown_option_fails() {
        assert_fails(
            parse(&["fmt", "--verbose"]),
            UsageError::UnknownOption("--verbose".to_string()),
            "unknown option '--verbose'",
        );
    }

    #[test]
    fn test_explain() {
        assert_eq!(
            parse(&["explain", "MUT-4"]),
            Ok(Command::Explain {
                code: "MUT-4".to_string()
            })
        );
    }

    #[test]
    fn test_explain_without_code_fails() {
        assert_fails(
            parse(&["explain"]),
            UsageError::CommandExpectsArgument {
                command: "explain",
                arg: "code",
            },
            "command 'explain' expects a code",
        );
    }

    #[test]
    fn test_explain_with_two_code_fails() {
        assert_fails(
            parse(&["explain", "MUT-4", "MUT-5"]),
            UsageError::UnexpectedArgument("MUT-5".to_string()),
            "unexpected argument 'MUT-5'",
        );
    }

    #[test]
    fn test_check_with_unknown_option_fails() {
        assert_fails(
            parse(&["check", "--verbose"]),
            UsageError::UnknownOption("--verbose".to_string()),
            "unknown option '--verbose'",
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_run_with_non_utf_8_program_args_fails() {
        let non_utf8 = invalid_utf8_os_string();
        let mut args: Vec<OsString> = ["run", "a.fm"].iter().map(OsString::from).collect();

        args.push(non_utf8.clone());

        assert_fails(
            parse_args(&args),
            UsageError::InvalidUtf8(non_utf8.to_string_lossy().to_string()),
            format!(
                "argument '{}' is not valid UTF-8",
                non_utf8.to_string_lossy(),
            )
            .as_str(),
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_non_utf8_subcommand_fails() {
        let non_utf8 = invalid_utf8_os_string();
        let args = vec![non_utf8.clone()];

        assert_fails(
            parse_args(&args),
            UsageError::InvalidUtf8(non_utf8.to_string_lossy().to_string()),
            format!(
                "argument '{}' is not valid UTF-8",
                non_utf8.to_string_lossy(),
            )
            .as_str(),
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_test_with_non_utf8_name_fails() {
        let non_utf8 = invalid_utf8_os_string();
        let mut args: Vec<OsString> = ["test", "--name"].iter().map(OsString::from).collect();

        args.push(non_utf8.clone());

        assert_fails(
            parse_args(&args),
            UsageError::InvalidUtf8(non_utf8.to_string_lossy().to_string()),
            format!(
                "argument '{}' is not valid UTF-8",
                non_utf8.to_string_lossy(),
            )
            .as_str(),
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_explain_with_non_utf8_code_fails() {
        let non_utf8 = invalid_utf8_os_string();
        let mut args: Vec<OsString> = ["explain"].iter().map(OsString::from).collect();

        args.push(non_utf8.clone());

        assert_fails(
            parse_args(&args),
            UsageError::InvalidUtf8(non_utf8.to_string_lossy().to_string()),
            format!(
                "argument '{}' is not valid UTF-8",
                non_utf8.to_string_lossy(),
            )
            .as_str(),
        );
    }
}
