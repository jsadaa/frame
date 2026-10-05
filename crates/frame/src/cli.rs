use std::{ffi::OsString, fmt, path::PathBuf};

/// The frame cli subcommands, each holding its own specific args
#[derive(Debug, PartialEq)]
pub enum Command {
    Run {
        target: Option<PathBuf>,
        args: Vec<OsString>,
    },
    Build {
        target: Option<PathBuf>,
        out: Option<PathBuf>,
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
#[derive(Debug, PartialEq)]
pub enum UsageError {
    InvalidUtf8(String),
    MissingCommand,
    UnknownCommand(String),
    InvalidHexadecimal(String),
    CommandExpectArg(String, String),
    OptionExpectValue(String),
    CommandTakesNoArg(String),
    OptionBeforeTarget(String, String),
    DuplicatedOption(String),
    DuplicatedTarget,
    DuplicatedCode,
    UnknownOption(String),
}

impl fmt::Display for UsageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UsageError::InvalidUtf8(arg) => write!(f, "invalid UTF-8: {arg:?}"),
            UsageError::MissingCommand => write!(f, "missing command"),
            UsageError::UnknownCommand(unknown) => write!(f, "unknown command: {unknown:?}"),
            UsageError::InvalidHexadecimal(hex) => write!(f, "invalid hexadecimal string: {hex:?}"),
            UsageError::CommandExpectArg(command, code) => {
                write!(f, "command {command:?} expects: {code:?}")
            }
            UsageError::OptionExpectValue(option) => {
                write!(f, "option {option:?} expects a value")
            }
            UsageError::CommandTakesNoArg(command) => {
                write!(f, "command {command:?} takes no arguments")
            }
            UsageError::OptionBeforeTarget(command, option) => {
                write!(
                    f,
                    "command {command:?} needs a target before option {option}"
                )
            }
            UsageError::DuplicatedOption(option) => {
                write!(f, "option {option:?} is duplicated")
            }
            UsageError::DuplicatedTarget => {
                write!(f, "target is duplicated")
            }
            UsageError::DuplicatedCode => {
                write!(f, "code is duplicated")
            }
            UsageError::UnknownOption(option) => {
                write!(f, "unknown option: {option}")
            }
        }
    }
}

/// Parse the args list and return the command on success and a usage error on failure
pub fn parse_args(args: &[OsString]) -> Result<Command, UsageError> {
    let first = args.first();

    if let Some(subcommand) = first {
        return match subcommand.to_str() {
            Some("run") => handle_run_command(args),
            Some("build") => handle_build_command(args),
            Some("check") => handle_check_command(args),
            Some("test") => handle_test_command(args),
            Some("fmt") => handle_fmt_command(args),
            Some("explain") => handle_explain_command(args),
            Some("help") => handle_help_command(args),
            Some("version") => Ok(Command::Version),
            Some(unknown) => Err(UsageError::UnknownCommand(unknown.to_owned())),
            None => Err(invalid_utf8(subcommand)),
        };
    }

    Err(UsageError::MissingCommand)
}

// Handlers

fn handle_run_command(args: &[OsString]) -> Result<Command, UsageError> {
    let target = get_positional_arg(args, 2);

    if target.clone().filter(arg_starts_with_hyphen).is_some() {
        return Err(UsageError::OptionBeforeTarget(
            "run".to_string(),
            target.unwrap().to_str().unwrap().to_string(), // we can unwrap because the guard is applied in `arg_starts_with_hyphen`
        ));
    }

    let target = target.map(PathBuf::from);
    let args: Vec<_> = args.iter().skip(2).cloned().collect();

    for arg in &args {
        if arg.to_str().is_none() {
            return Err(UsageError::InvalidUtf8(arg.to_string_lossy().to_string()));
        }
    }

    Ok(Command::Run { target, args })
}

fn handle_build_command(args: &[OsString]) -> Result<Command, UsageError> {
    let mut target: Option<PathBuf> = None;
    let mut out: Option<&OsString> = None;

    let mut iter = args.iter().skip(1);

    while let Some(arg) = iter.next() {
        match arg.to_str() {
            Some("-o") => {
                out = iter.next();
                if out.is_none() {
                    return Err(UsageError::OptionExpectValue("-o".to_string()));
                }

                if !arg_is_unique(args, "-o") {
                    return Err(UsageError::DuplicatedOption("-o".to_string()));
                }
            }
            Some(option) if option.starts_with("-") => {
                return Err(UsageError::UnknownOption(option.to_string()));
            }
            Some(_) => {
                target = Some(PathBuf::from(arg));
            }
            None => return Err(invalid_utf8(arg)),
        }
    }

    Ok(Command::Build {
        target,
        out: out.map(PathBuf::from),
    })
}

fn handle_check_command(args: &[OsString]) -> Result<Command, UsageError> {
    if let Some(arg) = args.iter().find(|a| arg_starts_with_hyphen(a)) {
        return Err(UsageError::UnknownOption(
            arg.clone().into_string().unwrap_or(String::new()),
        ));
    }

    if args.len() > 2 {
        return Err(UsageError::DuplicatedTarget);
    }

    let target = get_positional_arg(args, 2).map(PathBuf::from);

    Ok(Command::Check { target })
}

fn handle_test_command(args: &[OsString]) -> Result<Command, UsageError> {
    let mut target: Option<PathBuf> = None;
    let mut filter: Option<&OsString> = None;
    let mut seed: Option<&OsString> = None;

    let mut iter = args.iter().skip(1);

    while let Some(arg) = iter.next() {
        match arg.to_str() {
            Some("--name") => {
                filter = iter.next();
                if filter.is_none() {
                    return Err(UsageError::OptionExpectValue("--name".to_string()));
                }

                if !arg_is_unique(args, "--name") {
                    return Err(UsageError::DuplicatedOption("--name".to_string()));
                }
            }
            Some("--seed") => {
                seed = iter.next();
                if seed.is_none() {
                    return Err(UsageError::OptionExpectValue("--seed".to_string()));
                }

                if !arg_is_unique(args, "--seed") {
                    return Err(UsageError::DuplicatedOption("--seed".to_string()));
                }
            }
            Some(option) if option.starts_with("-") => {
                return Err(UsageError::UnknownOption(option.to_string()));
            }
            Some(_) => {
                target = Some(PathBuf::from(arg));
            }
            None => return Err(invalid_utf8(arg)),
        }
    }

    match seed {
        Some(s) => parse_hex(s.clone()).map(|s| Command::Test {
            target,
            filter: filter.and_then(|f| f.to_str().map(String::from)),
            seed: Some(s),
        }),
        None => Ok(Command::Test {
            target,
            filter: filter.and_then(|f| f.to_str().map(String::from)),
            seed: None,
        }),
    }
}

fn handle_fmt_command(args: &[OsString]) -> Result<Command, UsageError> {
    let paths: Vec<PathBuf> = args
        .iter()
        .skip(1)
        .filter(|arg| !arg_starts_with_hyphen(arg))
        .cloned()
        .map(PathBuf::from)
        .collect();

    match arg_exists(args, "--check") {
        true if !arg_is_unique(args, "--check") => {
            Err(UsageError::DuplicatedOption("--check".to_string()))
        }
        true => Ok(Command::Fmt { check: true, paths }),
        false => Ok(Command::Fmt {
            check: false,
            paths,
        }),
    }
}

fn handle_explain_command(args: &[OsString]) -> Result<Command, UsageError> {
    if args.len() > 2 {
        return Err(UsageError::DuplicatedCode);
    }

    let code = get_positional_arg(args, 2);

    if code.is_none() {
        return Err(UsageError::CommandExpectArg(
            "explain".to_string(),
            "code".to_string(),
        ));
    }

    let original = code.clone().unwrap();
    let checked = code.and_then(|c| c.to_str().map(String::from));

    match checked {
        Some(code) => Ok(Command::Explain {
            code: code.to_string(),
        }),
        None => Err(invalid_utf8(&original)),
    }
}

fn handle_help_command(args: &[OsString]) -> Result<Command, UsageError> {
    if args.iter().skip(1).take(1).next().is_some() {
        return Err(UsageError::CommandTakesNoArg("help".to_string()));
    }

    Ok(Command::Help)
}

// Utilities

fn get_positional_arg(args: &[OsString], pos: usize) -> Option<OsString> {
    args.iter().skip(pos - 1).take(1).next().cloned()
}

fn arg_exists(args: &[OsString], arg: &str) -> bool {
    args.iter()
        .find(|a| a.to_str().is_some_and(|a| a == arg))
        .is_some()
}

fn parse_hex(arg: OsString) -> Result<u64, UsageError> {
    match arg.to_str() {
        Some(arg) => {
            if !(1usize..=16usize).contains(&arg.chars().count())
                || !arg.chars().all(|c| c.is_ascii_hexdigit())
            {
                return Err(UsageError::InvalidHexadecimal(String::from(arg)));
            }

            u64::from_str_radix(arg, 16)
                .map_err(|_| UsageError::InvalidHexadecimal(String::from(arg)))
        }
        None => Err(invalid_utf8(&arg)),
    }
}

fn arg_starts_with_hyphen(arg: &OsString) -> bool {
    arg.to_str().is_some_and(|arg| arg.starts_with("-"))
}

fn arg_is_unique(args: &[OsString], option: &str) -> bool {
    args.iter()
        .filter(|a| a.to_str().is_some_and(|a| a == option))
        .count()
        == 1
}

fn invalid_utf8(arg: &OsString) -> UsageError {
    UsageError::InvalidUtf8(String::from_utf8_lossy(arg.as_encoded_bytes()).into_owned())
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
