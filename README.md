# Frame

## What is Frame?

Frame is an experimental compiled and typed application language, trying to find its way between rigour and pragmatism.

- Sum types, declared failures, neither null nor garbage collector;
- No lifetime management, no memory management in the code, scripts that execute directly.

One writes the logic: values, decisions, rules. The rest is up to execution.

## Principles

1. Values, nothing but values. A datum has no address. Assigning it, passing it, or returning it gives an independent copy, which costs O(1): the content is shared, then copied once, on first modification. There are no pointers, no references, no nulls, no mutable global variables, no mutable global state, no hidden side effects, no lifetime management.

2. The outside world is declared and provided. A function announces the capabilities it uses, and the context provides the implementation. The business code ignores where the time comes from, where the files are, and which database responds.

3. A failure is a value. A function declares its failures; the caller propagates them or handles them, always visibly. A bug is not a failure: it stops the program.

4. A signature says all that matters. Parameters, result, capabilities, failures, preconditions and business refusals. A function is understood by its body and by the signatures of the functions it calls. Inference remains local.

5. What can surprise is marked. A mutation, a failure that passes, a resource. What the compiler forbids does not need to be marked.

6. The validity depends only on written rules. A program is valid or not according to rules stated in the specification, never according to the precision of an analysis. Each compiler refusal cites its rule, its reason and a correction.

7. Everything is a type. The domain is modeled by types: records, sum types, distinct types, refinements. A type carries its methods, and polymorphism passes through interfaces. A capability is itself an interface, whose context provides the implementation.

8. One way only. One word per concept, one construction per intention, one formatter without option. A feature that duplicates another does not enter.

9. Magic is allowed if it is predictable. An implicit convenience is admitted under three conditions: it fits in a one-line rule, it never changes the meaning, and the tool can show it.

10. No meta-language. No macros, no attributes, no preprocessor, no lifetime annotations. What the language does not say, a library says, with the same words.

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

## Project structure

Frame is organized as a Cargo workspace with the following crates:

- **frame**: the main crate, handling the cli
- **frame_driver**: the driver crate, handling orchestration of the compilation process
- **frame_codegen**: the codegen crate, handling code generation
- **frame_ir**: the ir crate, handling the intermediate representation of the code
- **frame_sema**: the sema crate, handling semantic analysis
- **frame_syntax**: the syntax crate, handling parsing and syntax analysis
- **frame_source**: the source crate, handling source files and positioning
- **frame_diagnostics**: the diagnostics crate, handling errors and warnings
- **frame_vm**: the vm crate, handling the virtual machine for executing Frame code
- **frame_bytecode**: the bytecode crate, handling the bytecode representation of Frame code
- **frame_fmt**: the fmt crate, handling formatting of Frame code

## License

Frame is licensed under the MIT License. See the [LICENSE](LICENSE) file for more information.
