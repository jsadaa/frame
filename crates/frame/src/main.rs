//! The `frame` command-line interface.

mod cli;

use crate::cli::Command;
use std::{
    io::{self, Write},
    process::ExitCode,
};

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
    /// The subcommand of this name is not implemented yet. Provisional: each subcommand
    /// loses this outcome at its milestone.
    NotImplementedYet(&'static str),
}

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let outcome = match cli::parse_args(&args) {
        Ok(command) => execute(&command),
        Err(usage_error) => Outcome::UsageError(usage_error),
    };

    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();

    let written = write_outcome(&outcome, &mut stdout, &mut stderr)
        .and_then(|()| stdout.flush())
        .and_then(|()| stderr.flush());

    exit_code(&outcome, written)
}

/// Writes the outcome: the help and the version on the standard output, every other
/// message of `frame` on the standard error (CLI-9).
fn write_outcome(outcome: &Outcome, out: &mut impl Write, err: &mut impl Write) -> io::Result<()> {
    match outcome {
        Outcome::Success(Some(text)) => writeln!(out, "{text}"),
        Outcome::UsageError(e) => writeln!(err, "error: {e}\n{HELP_HINT}"),
        Outcome::NotImplementedYet(name) => writeln!(err, "'frame {name}' is not implemented yet"),
        Outcome::Success(None) => Ok(()),
    }
}

/// Returns the exit code of the outcome, given the result of writing it. The table of
/// CLI-9 lives here, and only here: a closed output gives 141; any other write error is
/// ignored.
fn exit_code(outcome: &Outcome, written: io::Result<()>) -> ExitCode {
    // 128 + 13: the code a shell shows for a program killed by SIGPIPE, a signal Rust ignores.
    if let Err(error) = written
        && error.kind() == io::ErrorKind::BrokenPipe
    {
        return ExitCode::from(141);
    }

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
        | Command::Explain { .. } => Outcome::NotImplementedYet(command.name()),
        Command::Help => Outcome::Success(Some(HELP.to_string())),
        Command::Version => Outcome::Success(Some(format!("frame {FRAME_VERSION}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::{ExitCode, FRAME_VERSION, HELP, Outcome, Write, cli, exit_code, write_outcome};
    use std::io;

    struct ClosedPipe;
    struct FailedDestination;

    impl Write for ClosedPipe {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Write for FailedDestination {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::Other.into())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn writes_each_outcome_to_its_output() {
        let version = format!("frame {FRAME_VERSION}\n");
        let help = format!("{HELP}\n");

        let cases: [(Outcome, &str, &str); 5] = [
            (Outcome::Success(None), "", ""),
            (Outcome::Success(Some(HELP.to_string())), &help, ""),
            (
                Outcome::Success(Some(format!("frame {FRAME_VERSION}"))),
                &version,
                "",
            ),
            (
                Outcome::UsageError(cli::UsageError::UnknownCommand("compile".to_string())),
                "",
                "error: unknown command 'compile'\nrun 'frame --help' for more information\n",
            ),
            (
                Outcome::NotImplementedYet("run"),
                "",
                "'frame run' is not implemented yet\n",
            ),
        ];

        for (outcome, expected_out, expected_err) in cases {
            let mut out = Vec::new();
            let mut err = Vec::new();
            write_outcome(&outcome, &mut out, &mut err).expect("writing to a Vec should not fail");

            let text_out = String::from_utf8(out).expect("the output should be UTF-8");
            let text_err = String::from_utf8(err).expect("the output should be UTF-8");

            assert_eq!(text_out, expected_out);
            assert_eq!(text_err, expected_err);
        }
    }

    #[test]
    fn broken_pipe_returns_141() {
        let outcome = Outcome::Success(Some(HELP.to_string()));
        let written = write_outcome(&outcome, &mut ClosedPipe, &mut ClosedPipe);

        assert_eq!(exit_code(&outcome, written), ExitCode::from(141),);
    }

    #[test]
    fn other_write_errors_keep_the_outcome_code() {
        let cases: [(Outcome, ExitCode); 2] = [
            (Outcome::Success(Some(HELP.to_string())), ExitCode::from(0)),
            (
                Outcome::UsageError(cli::UsageError::UnknownCommand("compile".to_string())),
                ExitCode::from(2),
            ),
        ];

        for (outcome, expected_code) in cases {
            let written = write_outcome(&outcome, &mut FailedDestination, &mut FailedDestination);
            let code = exit_code(&outcome, written);
            assert_eq!(code, expected_code);
        }
    }
}
