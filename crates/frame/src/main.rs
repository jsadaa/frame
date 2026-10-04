//! The `frame` command-line interface.

mod cli;

fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    match cli::parse_args(&args) {
        Ok(command) => println!("{command:?}"),
        Err(e) => eprintln!("{e}"),
    }
}
