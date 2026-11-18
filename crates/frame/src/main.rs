//! The `frame` command-line interface.

mod cli;

use frame_diagnostics::{Diagnostic, render};
use frame_driver::{Target, TargetError, check_script, resolve_target};
use frame_source::SourceMap;

use crate::cli::Command;
use std::{
    io::{self, Write},
    path::Path,
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

/// How a command ends, which decides what `frame` prints and its exit code (CLI-9).
enum Outcome {
    /// The command succeeded. Its text, if any, goes to standard output.
    Success(Option<String>),
    /// The command line is invalid.
    UsageError(cli::UsageError),
    /// The target cannot be worked on. This is a usage error too.
    TargetError(TargetError),
    /// The program is rejected. The text holds its rendered diagnostics.
    Refused(String),
    /// The named subcommand is not implemented yet.
    // TODO: remove once each subcommand is implemented, at its milestone.
    NotImplementedYet(&'static str),
    /// The named subcommand is not implemented yet on a package.
    // TODO(J19): remove once packages are supported.
    PackageNotImplementedYet(&'static str),
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

/// Writes the outcome. The help and the version go to standard output, every other message
/// to standard error (CLI-9).
fn write_outcome(outcome: &Outcome, out: &mut impl Write, err: &mut impl Write) -> io::Result<()> {
    match outcome {
        Outcome::Success(Some(text)) => writeln!(out, "{text}"),
        Outcome::Success(None) => Ok(()),
        Outcome::UsageError(e) => writeln!(err, "error: {e}\n{HELP_HINT}"),
        Outcome::TargetError(e) => writeln!(err, "error: {e}\n{HELP_HINT}"),
        // A rendering already ends with `\n`.
        Outcome::Refused(diagnostics) => write!(err, "{diagnostics}"),
        Outcome::NotImplementedYet(name) => writeln!(err, "'frame {name}' is not implemented yet"),
        Outcome::PackageNotImplementedYet(name) => {
            writeln!(err, "'frame {name}' on a package is not implemented yet")
        }
    }
}

/// Returns the exit code for `outcome`, given the result of writing it.
///
/// This is the only place that maps outcomes to the codes of CLI-9. A closed output gives
/// 141, and any other write error is ignored.
fn exit_code(outcome: &Outcome, written: io::Result<()>) -> ExitCode {
    // 141 is 128 + 13: what a shell reports for a program killed by SIGPIPE, which Rust
    // ignores.
    if let Err(error) = written
        && error.kind() == io::ErrorKind::BrokenPipe
    {
        return ExitCode::from(141);
    }

    match outcome {
        Outcome::Success(_) => ExitCode::from(0),
        Outcome::NotImplementedYet(_)
        | Outcome::PackageNotImplementedYet(_)
        | Outcome::Refused(_) => ExitCode::from(1),
        Outcome::UsageError(_) | Outcome::TargetError(_) => ExitCode::from(2),
    }
}

fn execute(command: &Command) -> Outcome {
    match command {
        Command::Run { .. }
        | Command::Build { .. }
        | Command::Test { .. }
        | Command::Fmt { .. }
        | Command::Explain { .. } => Outcome::NotImplementedYet(command.name()),
        Command::Check { target } => check(target.as_deref()),
        Command::Help => Outcome::Success(Some(HELP.to_string())),
        Command::Version => Outcome::Success(Some(format!("frame {FRAME_VERSION}"))),
    }
}

fn check(target: Option<&Path>) -> Outcome {
    let path = match resolve_target(target) {
        Ok(Target::Script(path)) => path,
        Ok(Target::Package(_)) => return Outcome::PackageNotImplementedYet("check"),
        Err(error) => return Outcome::TargetError(error),
    };

    match check_script(&path) {
        Ok(session) if session.diagnostics().is_empty() => Outcome::Success(None),
        Ok(session) => Outcome::Refused(render_diagnostics(
            session.diagnostics(),
            session.source_map(),
        )),
        Err(error) => Outcome::TargetError(error),
    }
}

/// Renders the diagnostics, separated by blank lines. Each rendering ends with `\n`, so
/// joining them with `\n` is enough.
fn render_diagnostics(diagnostics: &[Diagnostic], source_map: &SourceMap) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| render(diagnostic, source_map))
        .collect::<Vec<String>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::{
        ExitCode, FRAME_VERSION, HELP, Outcome, TargetError, Write, cli, exit_code,
        render_diagnostics, write_outcome,
    };
    use frame_diagnostics::{Diagnostic, Prefix, Primary, RuleCode};
    use frame_source::{SourceMap, Span};
    use std::{io, path::PathBuf};

    const UNEXPECTED_CHARACTER: RuleCode = RuleCode::new(Prefix::Lex, 24);

    const RENDERED_DIAGNOSTIC: &str = concat!(
        "error[CNF-6]: source file is too large\n",
        " --> big.fm\n",
        "  |\n",
        "  = reason: the limit is 16 MiB (16777216 bytes)\n",
    );

    fn missing_target() -> Outcome {
        Outcome::TargetError(TargetError::NotFound {
            path: PathBuf::from("missing.fm"),
        })
    }

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

        let cases: [(Outcome, &str, &str); 8] = [
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
            (
                missing_target(),
                "",
                "error: target 'missing.fm' does not exist\nrun 'frame --help' for more information\n",
            ),
            (
                Outcome::Refused(RENDERED_DIAGNOSTIC.to_string()),
                "",
                RENDERED_DIAGNOSTIC,
            ),
            (
                Outcome::PackageNotImplementedYet("check"),
                "",
                "'frame check' on a package is not implemented yet\n",
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
    fn diagnostics_are_separated_by_a_single_blank_line() {
        let mut source_map = SourceMap::new();
        let file = source_map.add(
            "demo.fm".to_string(),
            "let a = #1\nlet b = #2\n".to_string(),
        );
        let diagnostics = [
            Diagnostic::new(
                UNEXPECTED_CHARACTER,
                "unexpected character `#`",
                Primary::span(Span::new(file, 8, 9)),
            ),
            Diagnostic::new(
                UNEXPECTED_CHARACTER,
                "unexpected character `#`",
                Primary::span(Span::new(file, 19, 20)),
            ),
        ];

        assert_eq!(
            render_diagnostics(&diagnostics, &source_map),
            concat!(
                "error[LEX-24]: unexpected character `#`\n",
                " --> demo.fm:1:9\n",
                "  |\n",
                "1 | let a = #1\n",
                "  |         ^\n",
                "\n",
                "error[LEX-24]: unexpected character `#`\n",
                " --> demo.fm:2:9\n",
                "  |\n",
                "2 | let b = #2\n",
                "  |         ^\n",
            )
        );
    }

    #[test]
    fn each_outcome_has_its_exit_code() {
        let cases: [(&str, Outcome, ExitCode); 6] = [
            ("success", Outcome::Success(None), ExitCode::from(0)),
            (
                "usage error",
                Outcome::UsageError(cli::UsageError::UnknownCommand("compile".to_string())),
                ExitCode::from(2),
            ),
            ("target error", missing_target(), ExitCode::from(2)),
            (
                "refused",
                Outcome::Refused(RENDERED_DIAGNOSTIC.to_string()),
                ExitCode::from(1),
            ),
            (
                "not implemented yet",
                Outcome::NotImplementedYet("run"),
                ExitCode::from(1),
            ),
            (
                "package not implemented yet",
                Outcome::PackageNotImplementedYet("check"),
                ExitCode::from(1),
            ),
        ];

        for (case, outcome, expected_code) in cases {
            assert_eq!(
                exit_code(&outcome, Ok(())),
                expected_code,
                "exit code of {case}"
            );
        }
    }

    #[test]
    fn broken_pipe_returns_141() {
        let outcome = Outcome::Success(Some(HELP.to_string()));
        let written = write_outcome(&outcome, &mut ClosedPipe, &mut ClosedPipe);

        assert_eq!(exit_code(&outcome, written), ExitCode::from(141));
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
