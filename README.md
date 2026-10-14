# Frame

## What is Frame?

Frame is an experimental compiled and typed application language, trying to find its way between rigour and pragmatism.

- Sum types, declared failures, neither null nor garbage collector;
- No lifetime management, no memory management in the code, scripts that execute directly.

One writes the logic: values, decisions, rules. The rest is up to execution.

## Status

The language is in an early stage of development.

The documentation of the language, its design and its specification, is not included at this stage: like the project, it is experimental, and will change as the implementation progresses.

## Building

```bash
cargo build --release
target/release/frame --help
```

The commands are listed by `frame --help`; none is implemented yet.

## Development

Run the checks before every commit:

```bash
scripts/check.sh
```

The script runs three steps, in order, and stops at the first failure:

1. `cargo fmt --all --check`: the code follows rustfmt's default style;
2. `cargo clippy --workspace --all-targets -- -D warnings`: no compiler or Clippy warning remains;
3. `cargo test --workspace`: every test passes.

The CI runs the same script on every push.

## Project structure

Frame is organized as a Cargo workspace with the following crates:

- **frame**: the command-line tool
- **frame_source**: source files, positions in their text, and loading a file from disk

## Use of AI

The implementation is written by hand: I write its logic myself, and I review every change before committing it.

I use AI for research and understanding, for assistance with specification, for planning and breaking the work into tickets, for code reviews, and for learning. On request, it also makes small edits: names, documentation, messages.

## License

Frame is licensed under the MIT License. See the [LICENSE](LICENSE) file for more information.
