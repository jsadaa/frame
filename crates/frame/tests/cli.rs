//! The `frame` command line interface integration tests.

use std::process::Command;

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
  --version                                    Show the toolchain version
";

const FRAME_VERSION: &str = env!("CARGO_PKG_VERSION");

struct Outcome {
    // Kept so that a failed assertion names the case.
    args: Vec<String>,
    stdout: String,
    stderr: String,
    code: Option<i32>,
}

fn execute_command(args: &[&str]) -> Outcome {
    let output = Command::new(env!("CARGO_BIN_EXE_frame"))
        .args(args)
        .output()
        .expect("the frame binary should start");

    Outcome {
        args: args.iter().map(ToString::to_string).collect(),
        stdout: String::from_utf8(output.stdout).expect("stdout should be UTF-8"),
        stderr: String::from_utf8(output.stderr).expect("stderr should be UTF-8"),
        code: output.status.code(),
    }
}

fn assert_accepted(outcome: Outcome, expected_stdout: &str) {
    let Outcome {
        args,
        stdout,
        stderr,
        code,
    } = outcome;

    assert_eq!(code, Some(0), "exit code of frame {args:?}");
    assert_eq!(stdout, expected_stdout, "stdout of frame {args:?}");
    assert_eq!(stderr, "", "stderr of frame {args:?}");
}

fn assert_refused(outcome: Outcome, expected_code: i32, expected_in_stderr: &str) {
    let Outcome {
        args,
        stdout,
        stderr,
        code,
    } = outcome;

    assert_eq!(code, Some(expected_code), "exit code of frame {args:?}");
    assert_eq!(stdout, "", "stdout of frame {args:?}");
    assert!(
        stderr.contains(expected_in_stderr),
        "stderr of frame {args:?} should contain {expected_in_stderr:?}: {stderr:?}"
    );
}

#[test]
fn help_lists_every_subcommand() {
    assert_accepted(execute_command(&["--help"]), HELP);
}

#[test]
fn version_displays_frame_current_version() {
    assert_accepted(
        execute_command(&["--version"]),
        &format!("frame {FRAME_VERSION}\n"),
    );
}

#[test]
fn missing_command_points_to_help() {
    assert_refused(execute_command(&[]), 2, "frame --help");
}

#[test]
fn unknown_command_is_a_usage_error() {
    assert_refused(execute_command(&["compile"]), 2, "compile");
}

#[test]
fn unknown_option_is_a_usage_error() {
    assert_refused(execute_command(&["check", "--verbose"]), 2, "--verbose");
}

#[test]
fn subcommands_are_not_implemented_yet() {
    let cases: [&[&str]; 6] = [
        &["run", "x.fm"],
        &["build"],
        &["check"],
        &["test"],
        &["fmt"],
        &["explain", "MUT-4"],
    ];

    for args in cases {
        assert_refused(execute_command(args), 1, "not implemented yet");
    }
}
