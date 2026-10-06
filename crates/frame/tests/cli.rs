//! The frame command line interface integration tests

use std::process::{Command, Output};

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

fn execute_command(args: &[&str], message: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_frame"))
        .args(args)
        .output()
        .expect(message)
}

#[test]
fn help_option_display_commands_information() {
    let output = execute_command(
        &["--help"],
        "the help option should output frame commands information",
    );

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert_eq!(stdout, HELP.to_string());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.is_empty());
}

#[test]
fn version_display_frame_current_version() {
    let output = execute_command(
        &["--version"],
        "the version option should output frame current version",
    );

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert_eq!(stdout, format!("frame {FRAME_VERSION}\n"));
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.is_empty());
}

#[test]
fn no_args_display_help_hint() {
    let output = execute_command(
        &[],
        "the frame command with no args should display the help hint",
    );

    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("frame --help"));
}

#[test]
fn unknown_command_fails() {
    let output = execute_command(
        &["compile"],
        "An unknown command should output an error containing the command",
    );

    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("compile"));
}

#[test]
fn unknown_option_fails() {
    let output = execute_command(
        &["check", "--verbose"],
        "An unknown option should output an error containing the option",
    );

    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("--verbose"));
}

#[test]
fn not_implemented_commands_output_not_implemented_yet() {
    let message = "A command which is not implemented yet should output 'not implemented yet'";
    let cases = [
        execute_command(&["run", "x.fm"], message),
        execute_command(&["build", ""], message),
        execute_command(&["check", ""], message),
        execute_command(&["test", ""], message),
        execute_command(&["fmt", ""], message),
        execute_command(&["explain", "MUT-4"], message),
    ];

    for output in cases {
        assert_eq!(output.status.code(), Some(1));
        let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
        assert!(stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
        assert!(stderr.contains("not implemented yet"));
    }
}
