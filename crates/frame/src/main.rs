//! The `frame` command-line interface.

mod cli;

fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    let _ = cli::parse_args(&args);
}
