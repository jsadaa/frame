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

fn execute_command(args: &[&str]) -> (String, String, Option<i32>) {
    let output = Command::new(env!("CARGO_BIN_EXE_frame"))
        .args(args)
        .output()
        .expect("the frame binary should start");

    (
        String::from_utf8(output.stdout).expect("stdout should be UTF-8"),
        String::from_utf8(output.stderr).expect("stderr should be UTF-8"),
        output.status.code(),
    )
}

fn assert_accepted(outcome: (String, String, Option<i32>), expected_content: &str) {
    let (stdout, stderr, code) = outcome;
    assert_eq!(
        code,
        Some(0),
        "the exit code should be 0. actual : {code:?}",
    );
    assert_eq!(
        stdout, expected_content,
        "the std output should match the expected content: {expected_content}"
    );
    assert!(stderr.is_empty(), "the std error should be empty");
}

fn assert_refused(
    outcome: (String, String, Option<i32>),
    expected_code: i32,
    expected_content: &str,
) {
    let (stdout, stderr, code) = outcome;
    assert_eq!(
        code,
        Some(expected_code),
        "the exit code should be 1 or 2. actual : {code:?}"
    );
    assert!(stdout.is_empty(), "the std output should be empty");
    assert!(
        stderr.contains(expected_content),
        "the std error should match the expected content: {expected_content}"
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
fn unknown_option_fails_is_a_usage_error() {
    assert_refused(execute_command(&["check", "--verbose"]), 2, "--verbose");
}

#[test]
fn subcommands_are_not_implemented_yet() {
    let cases = [
        &["run", "x.fm"],
        &["build", ""],
        &["check", ""],
        &["test", ""],
        &["fmt", ""],
        &["explain", "MUT-4"],
    ];

    for args in cases {
        assert_refused(execute_command(args), 1, "not implemented yet");
    }
}
