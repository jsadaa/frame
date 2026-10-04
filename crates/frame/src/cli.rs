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

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Command::Run { target, args } => write!(f, "run: target {target:?}, args {args:?}"),
            Command::Build { target, out } => write!(f, "build: target {target:?}, out {out:?}"),
            Command::Check { target } => write!(f, "check: target {target:?}"),
            Command::Fmt { check, paths } => write!(f, "fmt: check {check}, paths {paths:?}"),
            Command::Test {
                target,
                filter,
                seed,
            } => write!(
                f,
                "test: target {target:?}, filter {filter:?}, seed {seed:?}"
            ),
            Command::Explain { code } => write!(f, "explain: code {code:?}"),
            Command::Help => write!(f, "help"),
            Command::Version => write!(f, "version"),
        }
    }
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
            target.unwrap().to_str().unwrap().to_string(), // we can unwrap because the guard is applied in `arg_is_option`
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

                if !option_is_unique(args, "-o") {
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

                if !option_is_unique(args, "--name") {
                    return Err(UsageError::DuplicatedOption("--name".to_string()));
                }
            }
            Some("--seed") => {
                seed = iter.next();
                if seed.is_none() {
                    return Err(UsageError::OptionExpectValue("--seed".to_string()));
                }

                if !option_is_unique(args, "--seed") {
                    return Err(UsageError::DuplicatedOption("--seed".to_string()));
                }
            }
            Some(option) if option.starts_with("--") => {
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
        true if !option_is_unique(args, "--check") => {
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

fn option_is_unique(args: &[OsString], option: &str) -> bool {
    args.iter()
        .filter(|a| a.to_str().is_some_and(|a| a == option))
        .count()
        == 1
}

fn invalid_utf8(arg: &OsString) -> UsageError {
    UsageError::InvalidUtf8(String::from_utf8_lossy(arg.as_encoded_bytes()).into_owned())
}
