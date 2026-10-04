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
pub enum UsageError {
    InvalidUtf8(String),
    MissingCommand,
    UnknownCommand(String),
    InvalidHexadecimal(String),
    CommandExpectArg(String, String),
    OptionExpectValue(String),
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
            Some("help") => Ok(Command::Help),
            Some("version") => Ok(Command::Version),
            Some(unknown) => Err(UsageError::UnknownCommand(unknown.to_owned())),
            None => Err(invalid_utf8(subcommand)),
        };
    }

    Err(UsageError::MissingCommand)
}

fn get_positional_arg(args: &[OsString], pos: usize) -> Option<OsString> {
    args.iter().skip(pos - 1).take(1).next().cloned()
}

fn get_option_value(args: &[OsString], option: &str) -> Option<OsString> {
    args.iter()
        .find(|a| a.to_str().is_some_and(|a| a == option))
        .cloned()
        .iter()
        .next()
        .cloned()
}

fn arg_exists(args: &[OsString], arg: &str) -> bool {
    args.iter()
        .find(|a| a.to_str().is_some_and(|a| a == arg))
        .is_some()
}

fn parse_hex(arg: OsString) -> Result<u64, UsageError> {
    match arg.to_str() {
        Some(arg) => {
            if !(1usize..16usize).contains(&arg.chars().count())
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

fn handle_run_command(args: &[OsString]) -> Result<Command, UsageError> {
    let target = get_positional_arg(args, 1).map(PathBuf::from);
    let args: Vec<_> = args.iter().skip(2).cloned().collect();

    Ok(Command::Run { target, args })
}

fn handle_build_command(args: &[OsString]) -> Result<Command, UsageError> {
    let target = get_positional_arg(args, 1).map(PathBuf::from);
    let out = get_option_value(args, "-o");

    if arg_exists(args, "-o") && out.is_none() {
        return Err(UsageError::OptionExpectValue("-o".to_string()));
    }

    Ok(Command::Build {
        target,
        out: out.map(PathBuf::from),
    })
}

fn handle_check_command(args: &[OsString]) -> Result<Command, UsageError> {
    let target = get_positional_arg(args, 1).map(PathBuf::from);

    Ok(Command::Check { target })
}

fn handle_test_command(args: &[OsString]) -> Result<Command, UsageError> {
    let target = get_positional_arg(args, 1).map(PathBuf::from);
    let filter = get_option_value(args, "--name");
    let seed = get_option_value(args, "--seed");

    if arg_exists(args, "--name") || filter.is_none() {
        return Err(UsageError::OptionExpectValue("--name".to_string()));
    }

    if arg_exists(args, "--seed") || seed.is_none() {
        return Err(UsageError::OptionExpectValue("--seed".to_string()));
    }

    match seed {
        Some(s) => parse_hex(s).map(|s| Command::Test {
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
    let paths: Vec<PathBuf> = args.iter().skip(2).cloned().map(PathBuf::from).collect();
    Ok(Command::Fmt {
        check: arg_exists(args, "--check"),
        paths,
    })
}

fn handle_explain_command(args: &[OsString]) -> Result<Command, UsageError> {
    let code = get_positional_arg(args, 1).and_then(|f| f.to_str().map(String::from));

    if code.is_none() {
        return Err(UsageError::CommandExpectArg(
            "explain".to_string(),
            "code".to_string(),
        ));
    }

    Ok(Command::Explain {
        code: code.unwrap(),
    })
}

fn invalid_utf8(arg: &OsString) -> UsageError {
    UsageError::InvalidUtf8(String::from_utf8_lossy(arg.as_encoded_bytes()).into_owned())
}
