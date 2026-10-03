# Frame

## What is Frame?

Frame is an experimental compiled and typed application language, trying to find its way between rigour and pragmatism.

- Sum types, declared failures, neither null nor garbage collector;
- No lifetime management, no memory management in the code, scripts that execute directly.

One writes the logic: values, decisions, rules. The rest is up to execution.

## Status

The language is in an early stage of development.

## Usage

```bash
# Build the project
cargo build --release
```

```bash
# Run the project
cargo run --release
```

```bash
# Run the tests
cargo test --workspace
```

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

- **frame**: the main crate, handling the cli

## License

Frame is licensed under the MIT License. See the [LICENSE](LICENSE) file for more information.
