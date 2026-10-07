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

/// How a command ends: what `frame` writes, and the exit code of CLI-9.
enum Outcome {
    /// The command succeeded; its text, if any, goes to the standard output.
    Success(Option<String>),
    /// The command line does not follow CLI-1.
    UsageError(cli::UsageError),
    /// The subcommand is not implemented yet. Provisional: each subcommand loses this
    /// message at its milestone.
    NotImplementedYet(&'static str),
}

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let outcome = match cli::parse_args(&args) {
        Ok(command) => execute(&command),
        Err(usage_error) => Outcome::UsageError(usage_error),
    };

    print_outcome(&outcome);
    exit_code(&outcome)
}

/// Writes the outcome: the help and the version on the standard output, every other
/// message of `frame` on the standard error (CLI-9).
fn print_outcome(outcome: &Outcome) {
    match outcome {
        Outcome::Success(Some(text)) => {
            println!("{text}");
        }
        Outcome::UsageError(e) => eprintln!(
            "error: {e}

{HELP_HINT}"
        ),
        Outcome::NotImplementedYet(m) => eprintln!("{m}"),
        Outcome::Success(None) => (),
    }
}

/// Returns the exit code of the outcome. The table of CLI-9 lives here, and only here.
fn exit_code(outcome: &Outcome) -> ExitCode {
    match outcome {
        Outcome::Success(_) => ExitCode::from(0),
        Outcome::NotImplementedYet(_) => ExitCode::from(1),
        Outcome::UsageError(_) => ExitCode::from(2),
    }
}

fn execute(command: &Command) -> Outcome {
    match command {
        Command::Run { .. }
        | Command::Build { .. }
        | Command::Test { .. }
        | Command::Check { .. }
        | Command::Fmt { .. }
        | Command::Explain { .. } => {
            Outcome::NotImplementedYet(format!("'frame {}' is not implemented yet", command.name()))
        }
        Command::Help => Outcome::Success(Some(HELP.to_string())),
        Command::Version => Outcome::Success(Some(format!("frame {FRAME_VERSION}"))),
    }
}
