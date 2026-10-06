//! The `frame` command-line interface.

mod cli;

use std::process::ExitCode;

use crate::cli::Command;

const HELP: &str = "Usage: frame <command> [arguments]

Commands:
  run [target [arguments...]]                  Compile in debug mode, then run
  build [target] [-o output]                   Build an executable in release mode
  check [target]                               Check the program and its tests
  test [target] [--name text] [--seed seed]    Run the tests
  fmt [--check] [paths...]                     Format source files
  explain code                                 Explain a rule

Options:
  --help                                       Show this help
  --version                                    Show the toolchain version";

const FRAME_VERSION: &str = env!("CARGO_PKG_VERSION");
const HELP_HINT: &str = "run 'frame --help' for more information";

#[allow(dead_code)]
enum Outcome {
    Success(Option<String>),
    Error,
    UsageError(cli::UsageError),
    ProgramOut,
    NotImplementedYet(String),
}

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let outcome = match cli::parse_args(&args) {
        Ok(command) => execute(command),
        Err(usage_error) => Outcome::UsageError(usage_error),
    };

    dispatch_to_std(&outcome);

    exit_code(&outcome)
}

fn dispatch_to_std(outcome: &Outcome) {
    match outcome {
        Outcome::Success(out) => {
            if let Some(out) = out {
                println!("{out}")
            }
        }
        Outcome::UsageError(e) => eprintln!(
            "error: {e}

{HELP_HINT}
            "
        ),
        Outcome::Error | Outcome::ProgramOut => {
            unimplemented!("program error and program out is not implemented yet ")
        }
        Outcome::NotImplementedYet(m) => eprintln!("{m}"),
    }
}

fn exit_code(outcome: &Outcome) -> ExitCode {
    match outcome {
        Outcome::Success(_) => ExitCode::from(0),
        Outcome::Error | Outcome::NotImplementedYet(_) => ExitCode::from(1),
        Outcome::UsageError(_) => ExitCode::from(2),
        Outcome::ProgramOut => unimplemented!("program out exit code is not implemented yet"),
    }
}

fn execute(command: Command) -> Outcome {
    match command {
        Command::Run { .. } => {
            Outcome::NotImplementedYet("'frame run' is not implemented yet".to_string())
        }
        Command::Build { .. } => {
            Outcome::NotImplementedYet("'frame build' is not implemented yet".to_string())
        }
        Command::Test { .. } => {
            Outcome::NotImplementedYet("'frame test' is not implemented yet".to_string())
        }
        Command::Check { .. } => {
            Outcome::NotImplementedYet("'frame check' is not implemented yet".to_string())
        }
        Command::Fmt { .. } => {
            Outcome::NotImplementedYet("'frame fmt' is not implemented yet".to_string())
        }
        Command::Explain { .. } => {
            Outcome::NotImplementedYet("'frame explain' is not implemented yet".to_string())
        }
        Command::Help => handle_help(),
        Command::Version => handle_version(),
    }
}

fn handle_help() -> Outcome {
    Outcome::Success(Some(HELP.to_string()))
}

fn handle_version() -> Outcome {
    Outcome::Success(Some(format!("frame {}", FRAME_VERSION)))
}
