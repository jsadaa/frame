use std::{
    ffi::{OsStr, OsString},
    fmt,
    path::PathBuf,
};

use crate::cli::UsageError::UnexpectedArgument;

/// The frame cli subcommands, each holding its own specific args
#[derive(Debug, PartialEq)]
pub enum Command {
    Run {
        target: Option<PathBuf>,
        program_args: Vec<OsString>,
    },
    Build {
        target: Option<PathBuf>,
        output: Option<PathBuf>,
    },
    Check {
        target: Option<PathBuf>,
    },
    Test {
        target: Option<PathBuf>,
        filter: Option<String>,
        seed: Option<u64>,
    },
    Fmt {
        check: bool,
        paths: Vec<PathBuf>,
    },
    Explain {
        code: String,
    },
    Help,
    Version,
}

/// A usage error from the input args
#[derive(Debug, PartialEq, Eq)]
pub enum UsageError {
    InvalidUtf8(String),
    MissingCommand,
    UnknownCommand(String),
    InvalidSeed(String),
    CommandExpectsArgument {
        command: &'static str,
        arg: &'static str,
    },
    OptionExpectsValue(&'static str),
    RepeatedOption(&'static str),
    UnexpectedArgument(String),
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

/// Parse the args list and return the command on success and a usage error on failure
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
        [target, program_args @ ..] if program_args.iter().any(|a| a.to_str().is_none()) => {
            for arg in program_args {
                if arg.to_str().is_none() {
                    return Err(UsageError::InvalidUtf8(arg.to_string_lossy().to_string()));
                }
            }

            unreachable!("at this step, a program arg have already failed")
        }
        [target, program_args @ ..] => (Some(PathBuf::from(target)), program_args.to_vec()),
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
    use crate::cli::{self, Command};
    use std::{ffi::OsString, path::PathBuf};

    fn args_to_os_strings(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
    }

    #[test]
    fn test_help() {
        let args = args_to_os_strings(&["help"]);
        let command = cli::parse_args(&args);
        assert_eq!(command, Ok(Command::Help));
    }

    #[test]
    fn test_version() {
        let args = args_to_os_strings(&["version"]);
        let command = cli::parse_args(&args);
        assert_eq!(command, Ok(Command::Version));
    }

    #[test]
    fn test_help_with_extra_arg_fails() {
        let args = args_to_os_strings(&["help", "whatever"]);
        let command = cli::parse_args(&args);

        assert_eq!(
            command,
            Err(cli::UsageError::CommandTakesNoArg("help".to_string()))
        );

        assert_eq!(
            format!("{}", command.unwrap_err()),
            "command \"help\" takes no arguments",
        )
    }

    #[test]
    fn test_parse_without_argument_fails() {
        let args = args_to_os_strings(&[]);
        let command = cli::parse_args(&args);

        assert_eq!(command, Err(cli::UsageError::MissingCommand));

        assert_eq!(format!("{}", command.unwrap_err()), "missing command",)
    }

    #[test]
    fn test_unknown_command_fails() {
        let args = args_to_os_strings(&["compile"]);
        let command = cli::parse_args(&args);

        assert_eq!(
            command,
            Err(cli::UsageError::UnknownCommand("compile".to_string()))
        );

        assert_eq!(
            format!("{}", command.unwrap_err()),
            "unknown command: \"compile\"",
        )
    }

    #[test]
    fn test_run_without_args() {
        let args = args_to_os_strings(&["run"]);
        let command = cli::parse_args(&args);
        let empty_vec: Vec<OsString> = Vec::new();

        assert_eq!(
            command,
            Ok(Command::Run {
                target: None,
                args: empty_vec
            })
        );
    }

    #[test]
    fn test_run_with_args() {
        let args = args_to_os_strings(&["run", "hello.fm", "Alice"]);
        let command = cli::parse_args(&args);
        let path = PathBuf::from("hello.fm");

        assert_eq!(
            command,
            Ok(Command::Run {
                target: Some(path),
                args: args_to_os_strings(&["Alice"]),
            })
        );
    }

    #[test]
    fn test_run_with_args_looking_like_options() {
        let args = args_to_os_strings(&["run", ".", "logs", "7", "--dry-run"]);
        let command = cli::parse_args(&args);
        let path = PathBuf::from(".");

        assert_eq!(
            command,
            Ok(Command::Run {
                target: Some(path),
                args: args_to_os_strings(&["logs", "7", "--dry-run"]),
            })
        );
    }

    #[test]
    fn test_run_with_option_before_target_fails() {
        let args = args_to_os_strings(&["run", "--fast", "x.fm"]);
        let command = cli::parse_args(&args);

        assert_eq!(
            command,
            Err(cli::UsageError::OptionBeforeTarget(
                "run".to_string(),
                "--fast".to_string()
            ))
        );

        assert_eq!(
            format!("{}", command.unwrap_err()),
            "command \"run\" needs a target before option --fast",
        )
    }

    #[test]
    fn test_build_with_ouput_and_no_target() {
        let args = args_to_os_strings(&["build", "-o", "out"]);
        let command = cli::parse_args(&args);
        let out = PathBuf::from("out");

        assert_eq!(
            command,
            Ok(Command::Build {
                target: None,
                out: Some(out)
            })
        );
    }

    #[test]
    fn test_build_with_ouput_and_target() {
        let args = args_to_os_strings(&["build", "app.fm", "-o", "out"]);
        let command = cli::parse_args(&args);
        let target = PathBuf::from("app.fm");
        let out = PathBuf::from("out");

        assert_eq!(
            command,
            Ok(Command::Build {
                target: Some(target),
                out: Some(out)
            })
        );
    }

    #[test]
    fn test_build_with_ouput_before_target() {
        let args = args_to_os_strings(&["build", "-o", "out", "app.fm"]);
        let command = cli::parse_args(&args);
        let target = PathBuf::from("app.fm");
        let out = PathBuf::from("out");

        assert_eq!(
            command,
            Ok(Command::Build {
                target: Some(target),
                out: Some(out)
            })
        );
    }

    #[test]
    fn test_build_with_ouput_and_no_value_fails() {
        let args = args_to_os_strings(&["build", "-o"]);
        let command = cli::parse_args(&args);

        assert_eq!(
            command,
            Err(cli::UsageError::OptionExpectValue("-o".to_string()))
        );

        assert_eq!(
            format!("{}", command.unwrap_err()),
            "option \"-o\" expects a value",
        )
    }

    #[test]
    fn test_build_with_duplicate_outputs_fails() {
        let args = args_to_os_strings(&["build", "-o", "a", "-o", "b"]);
        let command = cli::parse_args(&args);

        assert_eq!(
            command,
            Err(cli::UsageError::DuplicatedOption("-o".to_string()))
        );

        assert_eq!(
            format!("{}", command.unwrap_err()),
            "option \"-o\" is duplicated",
        )
    }

    #[test]
    fn test_check_with_target() {
        let args = args_to_os_strings(&["check", "app.fm"]);
        let command = cli::parse_args(&args);
        let out = PathBuf::from("app.fm");

        assert_eq!(command, Ok(Command::Check { target: Some(out) }));
    }

    #[test]
    fn test_check_without_target() {
        let args = args_to_os_strings(&["check"]);
        let command = cli::parse_args(&args);

        assert_eq!(command, Ok(Command::Check { target: None }));
    }

    #[test]
    fn test_check_with_duplicated_target_fails() {
        let args = args_to_os_strings(&["check", "a.fm", "b.fm"]);
        let command = cli::parse_args(&args);

        assert_eq!(command, Err(cli::UsageError::DuplicatedTarget));
        assert_eq!(format!("{}", command.unwrap_err()), "target is duplicated",)
    }

    #[test]
    fn test_test() {
        let args = args_to_os_strings(&["test", "--name", "parse", "--seed", "7c1f03a9e4b2d856"]);
        let command = cli::parse_args(&args);

        assert_eq!(
            command,
            Ok(Command::Test {
                target: None,
                filter: Some("parse".to_string()),
                seed: Some(0x7c1f03a9e4b2d856),
            })
        );
    }

    #[test]
    fn test_test_name_option_takes_next_arg() {
        let args = args_to_os_strings(&["test", "--name", "--seed"]);
        let command = cli::parse_args(&args);

        assert_eq!(
            command,
            Ok(Command::Test {
                target: None,
                filter: Some("--seed".to_string()),
                seed: None,
            })
        );
    }

    #[test]
    fn test_test_uppercas_seed() {
        let args = args_to_os_strings(&["test", "--seed", "FF"]);
        let command = cli::parse_args(&args);

        assert_eq!(
            command,
            Ok(Command::Test {
                target: None,
                filter: None,
                seed: Some(255),
            })
        );
    }

    #[test]
    fn test_test_with_invalid_seeds_fails() {
        let invalid: Vec<&str> = vec!["", "7c1f03a9e4b2d8562", "+ff", "0xff", "g1"];

        for seed in invalid {
            let args = args_to_os_strings(&["test", "--seed", seed]);
            let command = cli::parse_args(&args);

            assert_eq!(
                command,
                Err(cli::UsageError::InvalidHexadecimal(seed.to_string()))
            );

            assert_eq!(
                format!("{}", command.unwrap_err()),
                format!("invalid hexadecimal string: \"{seed}\""),
            )
        }
    }

    #[test]
    fn test_fmt() {
        let args = args_to_os_strings(&["fmt"]);
        let command = cli::parse_args(&args);
        let empty_vec: Vec<PathBuf> = Vec::new();

        assert_eq!(
            command,
            Ok(Command::Fmt {
                check: false,
                paths: empty_vec
            })
        );
    }

    #[test]
    fn test_fmt_with_check_flag() {
        let args = args_to_os_strings(&["fmt", "--check"]);
        let command = cli::parse_args(&args);
        let empty_vec: Vec<PathBuf> = Vec::new();

        assert_eq!(
            command,
            Ok(Command::Fmt {
                check: true,
                paths: empty_vec
            })
        );
    }

    #[test]
    fn test_fmt_paths_only() {
        let args = args_to_os_strings(&["fmt", "a.fm", "src"]);
        let command = cli::parse_args(&args);
        let paths: Vec<PathBuf> = args_to_os_strings(&["a.fm", "src"])
            .into_iter()
            .map(PathBuf::from)
            .collect();

        assert_eq!(
            command,
            Ok(Command::Fmt {
                check: false,
                paths,
            })
        );
    }

    #[test]
    fn test_fmt_with_path_before_flag() {
        let args = args_to_os_strings(&["fmt", "src", "--check"]);
        let command = cli::parse_args(&args);
        let paths: Vec<PathBuf> = args_to_os_strings(&["src"])
            .into_iter()
            .map(PathBuf::from)
            .collect();

        assert_eq!(command, Ok(Command::Fmt { check: true, paths }));
    }

    #[test]
    fn test_fmt_with_two_flags_fails() {
        let args = args_to_os_strings(&["fmt", "--check", "--check"]);
        let command = cli::parse_args(&args);

        assert_eq!(
            command,
            Err(cli::UsageError::DuplicatedOption("--check".to_string()))
        );

        assert_eq!(
            format!("{}", command.unwrap_err()),
            "option \"--check\" is duplicated",
        )
    }

    #[test]
    fn test_explain() {
        let args = args_to_os_strings(&["explain", "MUT-4"]);
        let command = cli::parse_args(&args);

        assert_eq!(
            command,
            Ok(Command::Explain {
                code: "MUT-4".to_string()
            })
        );
    }

    #[test]
    fn test_explain_without_code() {
        let args = args_to_os_strings(&["explain"]);
        let command = cli::parse_args(&args);

        assert_eq!(
            command,
            Err(cli::UsageError::CommandExpectArg(
                "explain".to_string(),
                "code".to_string(),
            ))
        );
    }

    #[test]
    fn test_explain_with_two_code_fails() {
        let args = args_to_os_strings(&["explain", "MUT-4", "MUT-5"]);
        let command = cli::parse_args(&args);

        assert_eq!(command, Err(cli::UsageError::DuplicatedCode));
    }

    #[test]
    fn test_check_with_unknown_option_fails() {
        let args = args_to_os_strings(&["check", "--verbose"]);
        let command = cli::parse_args(&args);

        assert_eq!(
            command,
            Err(cli::UsageError::UnknownOption("--verbose".to_string()))
        );

        assert_eq!(
            format!("{}", command.unwrap_err()),
            "unknown option: --verbose",
        )
    }

    #[test]
    #[cfg(unix)]
    fn test_program_with_non_utf_8_args_fails() {
        use std::os::unix::ffi::OsStringExt;

        let non_utf8 = OsString::from_vec(vec![0x66, 0xFF]);
        let mut args = args_to_os_strings(&["run", "a.fm"]);
        args.push(non_utf8.clone());

        let command = cli::parse_args(&args);

        assert_eq!(
            command,
            Err(cli::UsageError::InvalidUtf8(
                non_utf8.to_string_lossy().to_string(),
            ))
        );

        assert_eq!(
            format!("{}", command.unwrap_err()),
            format!(
                "invalid UTF-8: \"{}\"",
                non_utf8.to_string_lossy().to_string()
            ),
        )
    }
}
