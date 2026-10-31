# Toy in Rust with melior

The [MLIR Toy tutorial](https://mlir.llvm.org/docs/Tutorials/Toy/), in Rust with [melior](https://github.com/mlir-rs/melior).

## Usage

```sh
nix develop   # LLVM and MLIR 23
just test

cargo run -- examples/ast.toy --emit=ast              # chapter 1
cargo run -- examples/codegen.toy --emit=mlir         # chapter 2
cargo run -- examples/codegen.toy --emit=mlir --opt   # chapters 3 and 4
cargo run -- examples/codegen.toy --emit=mlir-affine  # chapter 5
cargo run -- examples/codegen.toy --emit=jit          # chapter 6
cargo run -- examples/struct_codegen.toy --emit=jit   # chapter 7
```

## Layout

- `toy`: the compiler and the `toyc` binary. The crate docs in
  `toy/src/lib.rs` map each chapter to its modules, and `toy/tests` has one
  test file per chapter.
- `melior-ext`: safe wrappers for parts of the MLIR C API that melior does not
  cover yet. All `unsafe` code lives here.
- `examples`: the Toy programs from the tutorial's tests.

## Differences from the C++ tutorial

Rust can only use MLIR's C API, so:

- The Toy dialect is defined at runtime in IRDL
  (`toy/src/dialect/toy.irdl.mlir`). Its verifiers and traits are attached from
  Rust, and operations print in MLIR's generic form.
- Functions and calls use `func.func` and `func.call`, with `toy.cast` on
  arguments, instead of `toy.func` and `toy.generic_call`.
- The canonicalization patterns, inliner, and shape inference interface are
  written in Rust.
