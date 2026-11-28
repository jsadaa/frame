//! Integration tests for the `frame` command line.

use std::{fs, path::Path, process::Command};

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

/// Runs `frame` from the crate's directory, so that relative targets and the paths that
/// diagnostics show are known in advance.
fn execute_command(args: &[&str]) -> Outcome {
    let output = Command::new(env!("CARGO_BIN_EXE_frame"))
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
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

fn assert_refused(outcome: Outcome, expected_code: i32, expected_stderr: &str) {
    let Outcome {
        args,
        stdout,
        stderr,
        code,
    } = outcome;

    assert_eq!(code, Some(expected_code), "exit code of frame {args:?}");
    assert_eq!(stdout, "", "stdout of frame {args:?}");
    assert_eq!(stderr, expected_stderr, "stderr of frame {args:?}");
}

fn usage_error(message: &str) -> String {
    format!("error: {message}\nrun 'frame --help' for more information\n")
}

fn not_implemented(name: &str) -> String {
    format!("'frame {name}' is not implemented yet\n")
}

fn package_not_implemented(name: &str) -> String {
    format!("'frame {name}' on a package is not implemented yet\n")
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
    assert_refused(execute_command(&[]), 2, &usage_error("missing command"));
}

#[test]
fn unknown_command_is_a_usage_error() {
    assert_refused(
        execute_command(&["compile"]),
        2,
        &usage_error("unknown command 'compile'"),
    );
}

#[test]
fn unknown_option_is_a_usage_error() {
    assert_refused(
        execute_command(&["check", "--verbose"]),
        2,
        &usage_error("unknown option '--verbose'"),
    );
}

#[test]
fn subcommands_are_not_implemented_yet() {
    let cases: [(&[&str], &str); 5] = [
        (&["run", "x.fm"], "run"),
        (&["build"], "build"),
        (&["test"], "test"),
        (&["fmt"], "fmt"),
        (&["explain", "MUT-4"], "explain"),
    ];

    for (args, name) in cases {
        assert_refused(execute_command(args), 1, &not_implemented(name));
    }
}

#[test]
fn check_accepts_a_valid_script() {
    assert_accepted(execute_command(&["check", "tests/fixtures/valid.fm"]), "");
}

#[test]
fn check_accepts_a_script_with_a_byte_order_mark() {
    assert_accepted(execute_command(&["check", "tests/fixtures/bom.fm"]), "");
}

#[test]
fn check_rejects_a_script_that_is_not_valid_utf8() {
    assert_refused(
        execute_command(&["check", "tests/fixtures/invalid_utf8.fm"]),
        1,
        concat!(
            "error[LEX-1]: source file is not valid UTF-8\n",
            " --> tests/fixtures/invalid_utf8.fm:2:5\n",
            "  |\n",
            "2 | let \u{FFFD} = 2\n",
            "  |     ^ invalid UTF-8 sequence\n",
        ),
    );
}

#[test]
fn check_rejects_a_script_too_large() {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("too_large.fm");
    fs::write(&path, vec![b'a'; 17 * 1024 * 1024]).expect("the large file should be written");
    let path = path
        .to_str()
        .expect("the path of the large file should be UTF-8");

    assert_refused(
        execute_command(&["check", path]),
        1,
        &format!(
            concat!(
                "error[CNF-6]: source file is too large\n",
                " --> {}\n",
                "  |\n",
                "  = reason: the limit is 16 MiB (16777216 bytes)\n",
            ),
            path
        ),
    );
}

// File permissions are set the Unix way.
#[cfg(unix)]
#[test]
fn check_rejects_an_unreadable_script_as_a_usage_error() {
    use std::os::unix::fs::PermissionsExt;

    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("unreadable.fm");
    // If a previous run left the file with no permissions, writing it again would fail.
    if path.exists() {
        fs::remove_file(&path).expect("the file of a previous run should be removed");
    }
    fs::write(&path, "let a = 1\n").expect("the unreadable file should be written");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000))
        .expect("the permissions of the unreadable file should change");
    let path = path
        .to_str()
        .expect("the path of the unreadable file should be UTF-8");

    assert_refused(
        execute_command(&["check", path]),
        2,
        &usage_error(&format!(
            "cannot read '{path}': Permission denied (os error 13)"
        )),
    );
}

// Directory permissions are set the Unix way.
#[cfg(unix)]
#[test]
fn check_rejects_a_script_in_an_unreadable_directory_as_a_usage_error() {
    use std::os::unix::fs::PermissionsExt;

    let directory = Path::new(env!("CARGO_TARGET_TMPDIR")).join("unreadable_directory");
    let path = directory.join("script.fm");
    // A previous run may have left the directory with no permissions.
    if directory.exists() {
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o755))
            .expect("the permissions of the directory of a previous run should be restored");
    }
    fs::create_dir_all(&directory).expect("the unreadable directory should be created");
    fs::write(&path, "let a = 1\n").expect("the script should be written");
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o000))
        .expect("the permissions of the unreadable directory should change");
    let path = path
        .to_str()
        .expect("the path of the script should be UTF-8");

    let outcome = execute_command(&["check", path]);
    // Restored before any assertion, since `cargo clean` cannot remove a directory with no
    // permissions.
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o755))
        .expect("the permissions of the unreadable directory should be restored");

    assert_refused(
        outcome,
        2,
        &usage_error(&format!(
            "cannot read '{path}': Permission denied (os error 13)"
        )),
    );
}

#[test]
fn check_rejects_a_missing_target() {
    assert_refused(
        execute_command(&["check", "missing.fm"]),
        2,
        &usage_error("target 'missing.fm' does not exist"),
    );
}

#[test]
fn check_rejects_a_file_without_the_fm_extension() {
    assert_refused(
        execute_command(&["check", "tests/fixtures/notes.txt"]),
        2,
        &usage_error("target 'tests/fixtures/notes.txt' is neither a '.fm' file nor a directory"),
    );
}

#[test]
fn check_on_a_directory_is_not_implemented_yet() {
    assert_refused(
        execute_command(&["check", "tests/fixtures"]),
        1,
        &package_not_implemented("check"),
    );
}

#[test]
fn check_without_a_target_is_not_implemented_yet() {
    assert_refused(
        execute_command(&["check"]),
        1,
        &package_not_implemented("check"),
    );
}

#[cfg(unix)]
mod closed_pipe {
    use std::{
        io,
        process::{Command, Stdio},
    };

    /// Returns a pipe whose read end is closed before `frame` starts, so that its first
    /// write always fails, with no race between the two processes.
    fn closed_pipe() -> Stdio {
        let (reader, writer) = io::pipe().expect("a pipe should open");
        drop(reader);

        Stdio::from(writer)
    }

    #[test]
    fn closed_stdout_exits_with_141() {
        let args = &["--help"];
        let std_out = closed_pipe();

        let output = Command::new(env!("CARGO_BIN_EXE_frame"))
            .args(args)
            .stdout(std_out)
            .output()
            .expect("the frame binary should start");

        assert_eq!(output.status.code(), Some(141));
        assert_eq!(String::from_utf8_lossy(&output.stderr), "");
    }

    #[test]
    fn closed_stderr_exits_with_141() {
        let args = &["compile"];
        let std_err = closed_pipe();

        let output = Command::new(env!("CARGO_BIN_EXE_frame"))
            .args(args)
            .stderr(std_err)
            .output()
            .expect("the frame binary should start");

        assert_eq!(output.status.code(), Some(141));
        assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    }
}
